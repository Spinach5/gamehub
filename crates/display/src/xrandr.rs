use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct X11Output {
    pub name: String,
    pub connected: bool,
    pub active: bool,
    pub current_mode: Option<(u32, u32, Option<u32>)>,
}

fn parse_geometry(token: &str) -> Option<(u32, u32)> {
    if !token.contains('+') {
        return None;
    }
    let (width, rest) = token.split_once('x')?;
    let height = rest.split('+').next()?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn parse_refresh(token: &str) -> Option<u32> {
    let rate = token.trim_end_matches(['*', '+']);
    let rate: f32 = rate.parse().ok()?;
    if rate.is_finite() && rate > 0.0 {
        Some(rate.round() as u32)
    } else {
        None
    }
}

pub fn parse_xrandr_output(text: &str) -> Vec<X11Output> {
    let mut outputs: Vec<X11Output> = Vec::new();
    let mut current: Option<usize> = None;

    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }

        if !line.chars().next().is_some_and(char::is_whitespace) {
            let mut parts = line.split_whitespace();
            let Some(name) = parts.next() else {
                current = None;
                continue;
            };
            let Some(state) = parts.next() else {
                current = None;
                continue;
            };
            if state != "connected" && state != "disconnected" {
                current = None;
                continue;
            }

            let mut geometry = None;
            for token in parts {
                if let Some(mode) = parse_geometry(token) {
                    geometry = Some(mode);
                    break;
                }
            }

            outputs.push(X11Output {
                name: name.to_string(),
                connected: state == "connected",
                active: geometry.is_some(),
                current_mode: geometry.map(|(width, height)| (width, height, None)),
            });
            current = Some(outputs.len() - 1);
            continue;
        }

        let Some(index) = current else {
            continue;
        };
        let output = &mut outputs[index];
        if !output.active {
            continue;
        }

        let mut parts = line.split_whitespace();
        let Some(mode_token) = parts.next() else {
            continue;
        };
        let Some((width, height)) = parse_geometry(&format!("{mode_token}+")) else {
            continue;
        };
        let Some((current_width, current_height, refresh)) = output.current_mode else {
            continue;
        };
        if width != current_width || height != current_height {
            continue;
        }

        let is_current =
            mode_token.contains('*') || line.split_whitespace().any(|token| token.contains('*'));
        if !is_current {
            continue;
        }

        let rate = parts.find_map(|token| {
            if token.contains('*') {
                parse_refresh(token)
            } else {
                None
            }
        });
        output.current_mode = Some((current_width, current_height, rate.or(refresh)));
    }

    outputs
}

pub fn list_x11_outputs() -> Vec<X11Output> {
    let output = match Command::new("xrandr").arg("--query").output() {
        Ok(output) => output,
        Err(error) => {
            tracing::warn!(%error, "cannot run xrandr");
            return Vec::new();
        }
    };
    if !output.status.success() {
        tracing::warn!(code = ?output.status.code(), "xrandr exited with failure");
        return Vec::new();
    }
    parse_xrandr_output(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"Screen 0: minimum 16 x 16, current 1920 x 1080, maximum 32767 x 32767
eDP-1 connected primary 1920x1080+0+0 (normal left inverted right x axis y axis) 340mm x 190mm
   1920x1080    143.88*+
   1440x1080    143.80
   1280x720     143.67
DP-1 disconnected (normal left inverted right x axis y axis)
HDMI-1 connected (normal left inverted right x axis y axis)
   2560x1440    120.00 +
"#;

    #[test]
    fn parses_active_output_with_fractional_refresh() {
        let outputs = parse_xrandr_output(SAMPLE);
        let edp = outputs
            .iter()
            .find(|output| output.name == "eDP-1")
            .unwrap();
        assert!(edp.connected);
        assert!(edp.active);
        assert_eq!(edp.current_mode, Some((1920, 1080, Some(144))));
    }

    #[test]
    fn includes_disconnected_and_inactive_outputs() {
        let outputs = parse_xrandr_output(SAMPLE);
        let dp = outputs.iter().find(|output| output.name == "DP-1").unwrap();
        assert!(!dp.connected);
        assert!(!dp.active);
        assert_eq!(dp.current_mode, None);

        let hdmi = outputs
            .iter()
            .find(|output| output.name == "HDMI-1")
            .unwrap();
        assert!(hdmi.connected);
        assert!(!hdmi.active);
        assert_eq!(hdmi.current_mode, None);
    }

    #[test]
    fn malformed_lines_are_ignored() {
        let outputs = parse_xrandr_output("garbage\n   123x\n\x00\n");
        assert!(outputs.is_empty());
    }

    #[test]
    fn multiple_connected_outputs_keep_their_own_modes() {
        let text = r#"Screen 0: minimum 16 x 16, current 3840 x 1080, maximum 32767 x 32767
eDP-1 connected primary 1920x1080+0+0 (normal) 340mm x 190mm
   1920x1080    60.00*+
HDMI-1 connected 1920x1080+1920+0 (normal) 500mm x 300mm
   1920x1080    120.00*+
"#;
        let outputs = parse_xrandr_output(text);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0].current_mode, Some((1920, 1080, Some(60))));
        assert_eq!(outputs[1].current_mode, Some((1920, 1080, Some(120))));
    }
}
