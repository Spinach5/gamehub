use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Arch {
    Win32,
    Win64,
}

impl Arch {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Win32 => "win32",
            Self::Win64 => "win64",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "win32" => Some(Self::Win32),
            "win64" => Some(Self::Win64),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RunnerKind {
    Wine,
    Proton,
}

impl RunnerKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wine => "wine",
            Self::Proton => "proton",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "wine" => Some(Self::Wine),
            "proton" => Some(Self::Proton),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunnerSource {
    System,
    Imported { original_path: PathBuf },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OverrideMode {
    Disabled,
    Native,
    Builtin,
    NativeBuiltin,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DllOverride {
    pub dll_name: String,
    pub mode: OverrideMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DisplayBackend {
    Auto,
    X11,
    Wayland,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GamescopeMode {
    Off,
    Auto,
    Forced,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayConfig {
    pub backend: DisplayBackend,
    pub monitor: Option<String>,
    pub resolution: Option<(u32, u32)>,
    pub refresh_rate: Option<u32>,
    pub fullscreen: bool,
    pub gamescope: GamescopeMode,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            backend: DisplayBackend::Auto,
            monitor: None,
            resolution: None,
            refresh_rate: None,
            fullscreen: true,
            gamescope: GamescopeMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Runner {
    pub id: Uuid,
    pub kind: RunnerKind,
    pub name: String,
    pub binary_path: PathBuf,
    pub root_dir: PathBuf,
    pub arch: Arch,
    pub source: RunnerSource,
    pub verified: bool,
    pub version_string: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Game {
    pub id: Uuid,
    pub name: String,
    pub executable_path: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub runner_id: Option<Uuid>,
    pub prefix_path: Option<PathBuf>,
    pub cover_path: Option<PathBuf>,
    pub arch: Arch,
    pub display: DisplayConfig,
    pub env_overrides: BTreeMap<String, String>,
    pub dll_overrides: Vec<DllOverride>,
    pub args: Vec<String>,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn arch_serializes_as_lowercase() {
        assert_eq!(serde_json::to_value(Arch::Win32).unwrap(), json!("win32"));
        assert_eq!(serde_json::to_value(Arch::Win64).unwrap(), json!("win64"));
    }

    #[test]
    fn arch_parse_is_exact() {
        assert_eq!(Arch::parse("win32"), Some(Arch::Win32));
        assert_eq!(Arch::parse("win64"), Some(Arch::Win64));
        assert_eq!(Arch::parse("Win64"), None);
        assert_eq!(Arch::parse("64"), None);
    }

    #[test]
    fn arch_as_str_matches_serde() {
        for arch in [Arch::Win32, Arch::Win64] {
            assert_eq!(serde_json::to_value(arch).unwrap(), json!(arch.as_str()));
        }
    }

    #[test]
    fn runner_source_shapes_are_fixed() {
        assert_eq!(
            serde_json::to_value(RunnerSource::System).unwrap(),
            json!({ "kind": "system" })
        );
        assert_eq!(
            serde_json::to_value(RunnerSource::Imported {
                original_path: PathBuf::from("/opt/GE-Proton9-1")
            })
            .unwrap(),
            json!({ "kind": "imported", "original_path": "/opt/GE-Proton9-1" })
        );
    }

    #[test]
    fn runner_source_round_trips() {
        let value = RunnerSource::Imported {
            original_path: PathBuf::from("/games/proton"),
        };
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<RunnerSource>(&encoded).unwrap(),
            value
        );
    }

    #[test]
    fn display_config_defaults_are_fixed() {
        let config = DisplayConfig::default();
        assert_eq!(config.backend, DisplayBackend::Auto);
        assert_eq!(config.monitor, None);
        assert_eq!(config.resolution, None);
        assert_eq!(config.refresh_rate, None);
        assert!(config.fullscreen);
        assert_eq!(config.gamescope, GamescopeMode::Auto);
    }

    #[test]
    fn display_config_json_shape_is_fixed() {
        let config = DisplayConfig {
            backend: DisplayBackend::Wayland,
            monitor: Some("eDP-1".into()),
            resolution: Some((1920, 1080)),
            refresh_rate: Some(144),
            fullscreen: true,
            gamescope: GamescopeMode::Forced,
        };
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            json!({
                "backend": "wayland",
                "monitor": "eDP-1",
                "resolution": [1920, 1080],
                "refresh_rate": 144,
                "fullscreen": true,
                "gamescope": "forced"
            })
        );
    }

    #[test]
    fn override_modes_serialize_as_snake_case() {
        for (mode, expected) in [
            (OverrideMode::Disabled, "disabled"),
            (OverrideMode::Native, "native"),
            (OverrideMode::Builtin, "builtin"),
            (OverrideMode::NativeBuiltin, "native_builtin"),
        ] {
            assert_eq!(serde_json::to_value(mode).unwrap(), json!(expected));
        }
    }

    #[test]
    fn dll_override_round_trips() {
        let value = DllOverride {
            dll_name: "steam_api64".into(),
            mode: OverrideMode::NativeBuiltin,
        };
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<DllOverride>(&encoded).unwrap(),
            value
        );
    }

    fn sample_runner() -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Proton,
            name: "GE-Proton9-1".into(),
            binary_path: PathBuf::from("/opt/GE-Proton9-1/proton"),
            root_dir: PathBuf::from("/opt/GE-Proton9-1"),
            arch: Arch::Win64,
            source: RunnerSource::Imported {
                original_path: PathBuf::from("/opt/GE-Proton9-1"),
            },
            verified: true,
            version_string: Some("GE-Proton9-1".into()),
        }
    }

    fn sample_game() -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "赛博朋克 2077".into(),
            executable_path: PathBuf::from("/games/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe"),
            working_dir: Some(PathBuf::from("/games/Cyberpunk 2077/bin/x64")),
            runner_id: Some(Uuid::new_v4()),
            prefix_path: None,
            cover_path: Some(PathBuf::from("/data/covers/abc.png")),
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            env_overrides: BTreeMap::from([
                ("WINEDEBUG".into(), "-all".into()),
                ("DXVK_HUD".into(), "fps".into()),
            ]),
            dll_overrides: vec![
                DllOverride {
                    dll_name: "d3d11".into(),
                    mode: OverrideMode::NativeBuiltin,
                },
                DllOverride {
                    dll_name: "winmm".into(),
                    mode: OverrideMode::Native,
                },
            ],
            args: vec!["-skipStartScreen".into(), "--windowed".into()],
            created_at: 1_700_000_000,
            last_played_at: Some(1_700_000_100),
        }
    }

    #[test]
    fn runner_round_trips() {
        let value = sample_runner();
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Runner>(&encoded).unwrap(), value);
    }

    #[test]
    fn game_round_trips_with_all_fields() {
        let value = sample_game();
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Game>(&encoded).unwrap(), value);
    }

    #[test]
    fn game_round_trips_with_cover_path() {
        let mut value = sample_game();
        value.cover_path = Some(PathBuf::from("/data/covers/cyberpunk.png"));
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(serde_json::from_str::<Game>(&encoded).unwrap(), value);
    }

    #[test]
    fn game_json_uses_fixed_field_names() {
        let value = serde_json::to_value(sample_game()).unwrap();
        for key in [
            "id",
            "name",
            "executable_path",
            "working_dir",
            "runner_id",
            "prefix_path",
            "cover_path",
            "arch",
            "display",
            "env_overrides",
            "dll_overrides",
            "args",
            "created_at",
            "last_played_at",
        ] {
            assert!(value.get(key).is_some(), "missing field {key}");
        }
    }
}
