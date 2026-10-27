use launcher_core::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn default_game_config_is_usable_without_database() {
    let game = Game {
        id: uuid::Uuid::nil(),
        name: "test".into(),
        executable_path: PathBuf::from("/tmp/test.exe"),
        working_dir: None,
        runner_id: None,
        prefix_path: None,
        cover_path: None,
        arch: Arch::Win64,
        display: DisplayConfig::default(),
        env_overrides: BTreeMap::new(),
        dll_overrides: Vec::new(),
        args: Vec::new(),
        created_at: 0,
        last_played_at: None,
    };

    let encoded = serde_json::to_value(&game).unwrap();
    assert_eq!(encoded["display"]["gamescope"], "auto");
    assert_eq!(encoded["arch"], "win64");
}

#[test]
fn unknown_enum_values_are_rejected() {
    assert!(serde_json::from_str::<Arch>("\"win16\"").is_err());
    assert!(serde_json::from_str::<RunnerKind>("\"umu\"").is_err());
    assert!(serde_json::from_str::<OverrideMode>("\"prefer_native\"").is_err());
    assert!(serde_json::from_str::<DisplayBackend>("\"drm\"").is_err());
    assert!(serde_json::from_str::<GamescopeMode>("\"always\"").is_err());
}
