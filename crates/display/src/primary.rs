use crate::{DisplayError, SessionType};
use std::collections::HashMap;
use std::process::Command;
use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

type MonitorInfo = (String, String, String, String);
type ModeProps = HashMap<String, OwnedValue>;
type Mode = (String, i32, i32, f64, f64, Vec<f64>, ModeProps);
type Monitor = (MonitorInfo, Vec<Mode>, ModeProps);
type LogicalMonitor = (i32, i32, f64, u32, bool, Vec<MonitorInfo>, ModeProps);
type DisplayState = (u32, Vec<Monitor>, Vec<LogicalMonitor>, ModeProps);

const DISPLAY_CONFIG: &str = "org.gnome.Mutter.DisplayConfig";
const DISPLAY_PATH: &str = "/org/gnome/Mutter/DisplayConfig";

/// Temporarily makes `connector` the primary monitor and returns the previous
/// primary, or `None` when it already was primary.
///
/// Wine's X11 driver places new windows on the XRandR primary output, so this
/// is how a selected monitor is applied to an X11 Wine/Proton game. Call it
/// again with the returned connector to restore the original layout.
pub fn set_primary_output(
    connector: &str,
    session: SessionType,
) -> Result<Option<String>, DisplayError> {
    match session {
        SessionType::X11 => set_primary_xrandr(connector),
        SessionType::Wayland => set_primary_mutter(connector),
        SessionType::Unknown => {
            if std::env::var_os("DISPLAY").is_some() {
                set_primary_mutter(connector).or_else(|error| {
                    tracing::debug!(%error, "Mutter DisplayConfig unavailable; trying xrandr");
                    set_primary_xrandr(connector)
                })
            } else {
                Err(DisplayError::Xrandr("no X display available".into()))
            }
        }
    }
}

fn set_primary_mutter(connector: &str) -> Result<Option<String>, DisplayError> {
    let connection = Connection::session()?;
    let state = get_mutter_state(&connection)?;
    let (serial, monitors, logical, _) = state;
    let modes = current_modes(&monitors);
    let original = primary_connector(&logical);

    if original.as_deref() == Some(connector) {
        return Ok(None);
    }

    let mut target_found = false;
    let mut config = Vec::with_capacity(logical.len());
    for (x, y, scale, transform, _primary, monitors_in_logical, _) in logical {
        let is_target = monitors_in_logical
            .iter()
            .any(|monitor| monitor.0 == connector);
        target_found |= is_target;

        let mut outputs = Vec::with_capacity(monitors_in_logical.len());
        for monitor in monitors_in_logical {
            let mode = modes.get(&monitor.0).ok_or_else(|| {
                DisplayError::Unexpected(format!("no current mode for {}", monitor.0))
            })?;
            outputs.push((monitor.0.clone(), mode.clone(), ModeProps::new()));
        }
        config.push((x, y, scale, transform, is_target, outputs));
    }

    if !target_found {
        return Err(DisplayError::MonitorNotFound(connector.to_string()));
    }

    connection.call_method(
        Some(DISPLAY_CONFIG),
        DISPLAY_PATH,
        Some(DISPLAY_CONFIG),
        "ApplyMonitorsConfig",
        &(serial, 1u32, config, ModeProps::new()),
    )?;

    let state = get_mutter_state(&connection)?;
    if primary_connector(&state.2).as_deref() == Some(connector) {
        tracing::info!(%connector, previous = ?original, "temporarily changed primary monitor");
    } else {
        tracing::warn!(%connector, "primary monitor did not change; it will be restored");
    }

    Ok(original)
}

fn get_mutter_state(connection: &Connection) -> Result<DisplayState, DisplayError> {
    let reply = connection.call_method(
        Some(DISPLAY_CONFIG),
        DISPLAY_PATH,
        Some(DISPLAY_CONFIG),
        "GetCurrentState",
        &(),
    )?;
    reply
        .body()
        .deserialize::<DisplayState>()
        .map_err(DisplayError::from)
}

fn current_modes(monitors: &[Monitor]) -> HashMap<String, String> {
    let mut modes = HashMap::new();
    for (info, mode_list, _) in monitors {
        for mode in mode_list {
            let current = mode
                .6
                .get("is-current")
                .and_then(|value| bool::try_from(value).ok())
                .unwrap_or(false);
            if current {
                modes.insert(info.0.clone(), mode.0.clone());
            }
        }
    }
    modes
}

fn primary_connector(logical: &[LogicalMonitor]) -> Option<String> {
    logical
        .iter()
        .find(|monitor| monitor.4)
        .and_then(|monitor| monitor.5.first())
        .map(|monitor| monitor.0.clone())
}

fn set_primary_xrandr(connector: &str) -> Result<Option<String>, DisplayError> {
    let outputs = crate::xrandr::list_xrandr_outputs();
    let original = outputs
        .iter()
        .find(|output| output.primary)
        .map(|output| output.name.clone());

    if original.as_deref() == Some(connector) {
        return Ok(None);
    }
    if !outputs
        .iter()
        .any(|output| output.connected && output.name == connector)
    {
        return Err(DisplayError::MonitorNotFound(connector.to_string()));
    }

    let output = Command::new("xrandr")
        .args(["--output", connector, "--primary"])
        .output()
        .map_err(|error| DisplayError::Xrandr(error.to_string()))?;
    if !output.status.success() {
        return Err(DisplayError::Xrandr(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    let outputs = crate::xrandr::list_xrandr_outputs();
    if outputs
        .iter()
        .any(|output| output.primary && output.name == connector)
    {
        tracing::info!(%connector, previous = ?original, "temporarily changed X11 primary monitor");
    } else {
        tracing::warn!(%connector, "xrandr did not change the primary monitor");
    }

    Ok(original)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "changes the primary monitor of the current session"]
    fn primary_round_trip() {
        let outputs = crate::list_outputs();
        let current = outputs
            .iter()
            .find(|output| output.primary)
            .expect("no primary output")
            .name
            .clone();
        let target = outputs
            .iter()
            .find(|output| output.connected && !output.primary)
            .expect("no secondary output")
            .name
            .clone();

        let previous = set_primary_output(&target, SessionType::Wayland).unwrap();
        assert_eq!(previous.as_deref(), Some(current.as_str()));
        assert!(
            crate::list_outputs()
                .iter()
                .any(|output| output.primary && output.name == target)
        );

        let previous = set_primary_output(&current, SessionType::Wayland).unwrap();
        assert_eq!(previous.as_deref(), Some(target.as_str()));
        assert!(
            crate::list_outputs()
                .iter()
                .any(|output| output.primary && output.name == current)
        );
    }
}
