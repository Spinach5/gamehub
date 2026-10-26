use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

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
}
