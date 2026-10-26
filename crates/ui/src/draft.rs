use launcher_core::{Arch, DisplayConfig, DllOverride, Game, OverrideMode};
use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvRow {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllRow {
    pub name: String,
    pub mode: OverrideMode,
}

#[derive(Debug, Clone)]
pub struct GameDraft {
    pub id: Uuid,
    pub is_new: bool,
    pub name: String,
    pub executable_path: Option<PathBuf>,
    pub working_dir: Option<PathBuf>,
    pub runner_id: Option<Uuid>,
    pub prefix_path: Option<PathBuf>,
    pub arch: Arch,
    pub display: DisplayConfig,
    pub resolution_width: String,
    pub resolution_height: String,
    pub refresh_rate: String,
    pub env_rows: Vec<EnvRow>,
    pub dll_rows: Vec<DllRow>,
    pub args_rows: Vec<String>,
    pub created_at: i64,
    pub last_played_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum DraftAction {
    Name(String),
    WorkingDir(Option<PathBuf>),
    ClearWorkingDir,
    Arch(Arch),
    Runner(Option<Uuid>),
    Prefix(Option<PathBuf>),
    ClearPrefix,
}

#[derive(Debug, Clone)]
pub enum EnvAction {
    Add,
    Remove(usize),
    Key { index: usize, key: String },
    Value { index: usize, value: String },
}

pub fn apply_env_action(rows: &mut Vec<EnvRow>, action: EnvAction) {
    match action {
        EnvAction::Add => rows.push(EnvRow {
            key: String::new(),
            value: String::new(),
        }),
        EnvAction::Remove(index) => {
            if index < rows.len() {
                rows.remove(index);
            }
        }
        EnvAction::Key { index, key } => {
            if let Some(row) = rows.get_mut(index) {
                row.key = key;
            }
        }
        EnvAction::Value { index, value } => {
            if let Some(row) = rows.get_mut(index) {
                row.value = value;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftError {
    pub field: &'static str,
    pub message: String,
}

impl DraftError {
    fn new(field: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

impl fmt::Display for DraftError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for DraftError {}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn parse_resolution(width: &str, height: &str) -> Result<Option<(u32, u32)>, DraftError> {
    let width = width.trim();
    let height = height.trim();
    match (width.is_empty(), height.is_empty()) {
        (true, true) => Ok(None),
        (true, false) | (false, true) => {
            Err(DraftError::new("分辨率", "宽和高必须同时填写或同时留空"))
        }
        (false, false) => {
            let width: u32 = width
                .parse()
                .map_err(|_| DraftError::new("分辨率", "宽度必须是正整数"))?;
            let height: u32 = height
                .parse()
                .map_err(|_| DraftError::new("分辨率", "高度必须是正整数"))?;
            if width == 0 || height == 0 {
                return Err(DraftError::new("分辨率", "宽和高必须大于 0"));
            }
            Ok(Some((width, height)))
        }
    }
}

fn parse_refresh(value: &str) -> Result<Option<u32>, DraftError> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let rate: u32 = value
        .parse()
        .map_err(|_| DraftError::new("刷新率", "必须是正整数"))?;
    if rate == 0 {
        return Err(DraftError::new("刷新率", "必须大于 0"));
    }
    Ok(Some(rate))
}

fn normalize_dll_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.to_ascii_lowercase().ends_with(".dll") {
        trimmed[..trimmed.len() - 4].trim_end().to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn env_rows_to_map(rows: &[EnvRow]) -> Result<BTreeMap<String, String>, DraftError> {
    let mut map = BTreeMap::new();
    for row in rows {
        let key = row.key.trim();
        if key.is_empty() && row.value.trim().is_empty() {
            continue;
        }
        if key.is_empty() {
            return Err(DraftError::new("环境变量", "变量名不能为空"));
        }
        if key == "WINEDLLOVERRIDES" {
            return Err(DraftError::new(
                "环境变量",
                "WINEDLLOVERRIDES 必须通过结构化 DLL override 配置",
            ));
        }
        launch::validate_env_key(key)
            .map_err(|error| DraftError::new("环境变量", error.to_string()))?;
        let previous = map.insert(key.to_string(), row.value.clone());
        if previous.is_some() {
            return Err(DraftError::new("环境变量", format!("变量 {key} 重复")));
        }
    }
    Ok(map)
}

fn dll_rows_to_overrides(rows: &[DllRow]) -> Result<Vec<DllOverride>, DraftError> {
    let mut overrides = Vec::new();
    let mut seen = HashSet::new();
    for row in rows {
        let name = normalize_dll_name(&row.name);
        if name.is_empty() {
            continue;
        }
        launch::validate_dll_name(&name)
            .map_err(|error| DraftError::new("DLL Override", error.to_string()))?;
        let key = name.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(DraftError::new("DLL Override", format!("DLL {name} 重复")));
        }
        overrides.push(DllOverride {
            dll_name: name,
            mode: row.mode,
        });
    }
    Ok(overrides)
}

impl GameDraft {
    pub fn new(executable_path: PathBuf) -> Self {
        let name = executable_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .unwrap_or("新游戏")
            .to_string();

        Self {
            id: Uuid::new_v4(),
            is_new: true,
            name,
            executable_path: Some(executable_path),
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            resolution_width: String::new(),
            resolution_height: String::new(),
            refresh_rate: String::new(),
            env_rows: Vec::new(),
            dll_rows: Vec::new(),
            args_rows: Vec::new(),
            created_at: now_unix(),
            last_played_at: None,
        }
    }

    pub fn from_game(game: &Game) -> Self {
        let (resolution_width, resolution_height) = game
            .display
            .resolution
            .map(|(width, height)| (width.to_string(), height.to_string()))
            .unwrap_or_default();

        Self {
            id: game.id,
            is_new: false,
            name: game.name.clone(),
            executable_path: Some(game.executable_path.clone()),
            working_dir: game.working_dir.clone(),
            runner_id: game.runner_id,
            prefix_path: game.prefix_path.clone(),
            arch: game.arch,
            display: game.display.clone(),
            resolution_width,
            resolution_height,
            refresh_rate: game
                .display
                .refresh_rate
                .map(|rate| rate.to_string())
                .unwrap_or_default(),
            env_rows: game
                .env_overrides
                .iter()
                .map(|(key, value)| EnvRow {
                    key: key.clone(),
                    value: value.clone(),
                })
                .collect(),
            dll_rows: game
                .dll_overrides
                .iter()
                .map(|entry| DllRow {
                    name: entry.dll_name.clone(),
                    mode: entry.mode,
                })
                .collect(),
            args_rows: game.args.clone(),
            created_at: game.created_at,
            last_played_at: game.last_played_at,
        }
    }

    pub fn apply(&mut self, action: DraftAction) {
        match action {
            DraftAction::Name(value) => self.name = value,
            DraftAction::WorkingDir(value) => self.working_dir = value,
            DraftAction::ClearWorkingDir => self.working_dir = None,
            DraftAction::Arch(value) => self.arch = value,
            DraftAction::Runner(value) => self.runner_id = value,
            DraftAction::Prefix(value) => self.prefix_path = value,
            DraftAction::ClearPrefix => self.prefix_path = None,
        }
    }

    pub fn to_game(&self) -> Result<Game, DraftError> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(DraftError::new("名称", "不能为空"));
        }

        let executable_path = self
            .executable_path
            .clone()
            .ok_or_else(|| DraftError::new("可执行文件", "未选择"))?;
        if !executable_path.is_absolute() {
            return Err(DraftError::new("可执行文件", "必须是绝对路径"));
        }

        if let Some(working_dir) = &self.working_dir {
            if working_dir.as_os_str().is_empty() || !working_dir.is_absolute() {
                return Err(DraftError::new("工作目录", "必须是绝对路径"));
            }
        }
        if let Some(prefix_path) = &self.prefix_path {
            if prefix_path.as_os_str().is_empty() || !prefix_path.is_absolute() {
                return Err(DraftError::new("Prefix", "必须是绝对路径"));
            }
        }

        let resolution = parse_resolution(&self.resolution_width, &self.resolution_height)?;
        let refresh_rate = parse_refresh(&self.refresh_rate)?;
        let mut display = self.display.clone();
        display.resolution = resolution;
        display.refresh_rate = refresh_rate;

        let env_overrides = env_rows_to_map(&self.env_rows)?;
        let dll_overrides = dll_rows_to_overrides(&self.dll_rows)?;
        let args = self
            .args_rows
            .iter()
            .map(|arg| arg.trim())
            .filter(|arg| !arg.is_empty())
            .map(str::to_owned)
            .collect();

        Ok(Game {
            id: self.id,
            name: name.to_string(),
            executable_path,
            working_dir: self.working_dir.clone(),
            runner_id: self.runner_id,
            prefix_path: self.prefix_path.clone(),
            arch: self.arch,
            display,
            env_overrides,
            dll_overrides,
            args,
            created_at: self.created_at,
            last_played_at: self.last_played_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, DisplayConfig, DllOverride, GamescopeMode, OverrideMode};
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn draft() -> GameDraft {
        GameDraft {
            id: Uuid::nil(),
            is_new: true,
            name: "Test Game".into(),
            executable_path: Some(PathBuf::from("/games/Test/game.exe")),
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            arch: Arch::Win64,
            display: DisplayConfig::default(),
            resolution_width: String::new(),
            resolution_height: String::new(),
            refresh_rate: String::new(),
            env_rows: Vec::new(),
            dll_rows: Vec::new(),
            args_rows: Vec::new(),
            created_at: 1_700_000_000,
            last_played_at: None,
        }
    }

    #[test]
    fn new_draft_uses_file_stem_as_name() {
        let draft = GameDraft::new(PathBuf::from("/games/Cyberpunk 2077.exe"));
        assert_eq!(draft.name, "Cyberpunk 2077");
        assert!(draft.is_new);
        assert_eq!(
            draft.executable_path,
            Some(PathBuf::from("/games/Cyberpunk 2077.exe"))
        );
    }

    #[test]
    fn minimal_draft_converts_to_game() {
        let game = draft().to_game().unwrap();
        assert_eq!(game.name, "Test Game");
        assert_eq!(game.display.resolution, None);
        assert_eq!(game.display.refresh_rate, None);
        assert!(game.env_overrides.is_empty());
        assert!(game.dll_overrides.is_empty());
        assert!(game.args.is_empty());
    }

    #[test]
    fn empty_name_is_rejected() {
        let mut draft = draft();
        draft.name = "   ".into();
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn missing_or_relative_executable_is_rejected() {
        let mut missing = draft();
        missing.executable_path = None;
        assert!(missing.to_game().is_err());

        let mut relative = draft();
        relative.executable_path = Some(PathBuf::from("game.exe"));
        assert!(relative.to_game().is_err());
    }

    #[test]
    fn resolution_requires_both_dimensions() {
        let mut draft = draft();
        draft.resolution_width = "1920".into();
        assert!(draft.to_game().is_err());

        draft.resolution_height = "1080".into();
        assert_eq!(
            draft.to_game().unwrap().display.resolution,
            Some((1920, 1080))
        );

        draft.resolution_width = "0".into();
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn refresh_rate_parses_and_validates() {
        let mut draft = draft();
        draft.refresh_rate = "144".into();
        assert_eq!(draft.to_game().unwrap().display.refresh_rate, Some(144));

        draft.refresh_rate = "0".into();
        assert!(draft.to_game().is_err());

        draft.refresh_rate = "abc".into();
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn empty_env_rows_are_skipped_and_duplicates_rejected() {
        let mut draft = draft();
        draft.env_rows = vec![
            EnvRow {
                key: "".into(),
                value: "".into(),
            },
            EnvRow {
                key: "DXVK_HUD".into(),
                value: "fps".into(),
            },
        ];
        let game = draft.to_game().unwrap();
        assert_eq!(game.env_overrides.len(), 1);
        assert_eq!(game.env_overrides["DXVK_HUD"], "fps");

        draft.env_rows.push(EnvRow {
            key: "DXVK_HUD".into(),
            value: "fps,frametimes".into(),
        });
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn manual_winedlloverrides_and_invalid_keys_are_rejected() {
        let mut draft = draft();
        draft.env_rows = vec![EnvRow {
            key: "WINEDLLOVERRIDES".into(),
            value: "x=n".into(),
        }];
        assert!(draft.to_game().is_err());

        draft.env_rows = vec![EnvRow {
            key: "BAD KEY".into(),
            value: "x".into(),
        }];
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn dll_names_are_normalized_and_duplicates_rejected() {
        let mut draft = draft();
        draft.dll_rows = vec![
            DllRow {
                name: "d3d11.dll".into(),
                mode: OverrideMode::NativeBuiltin,
            },
            DllRow {
                name: "  winmm ".into(),
                mode: OverrideMode::Native,
            },
            DllRow {
                name: "".into(),
                mode: OverrideMode::Native,
            },
        ];
        let game = draft.to_game().unwrap();
        assert_eq!(
            game.dll_overrides,
            vec![
                DllOverride {
                    dll_name: "d3d11".into(),
                    mode: OverrideMode::NativeBuiltin
                },
                DllOverride {
                    dll_name: "winmm".into(),
                    mode: OverrideMode::Native
                }
            ]
        );

        draft.dll_rows.push(DllRow {
            name: "D3D11".into(),
            mode: OverrideMode::Builtin,
        });
        assert!(draft.to_game().is_err());
    }

    #[test]
    fn invalid_dll_names_are_rejected() {
        for name in ["../evil", "bad name", "a/b"] {
            let mut draft = draft();
            draft.dll_rows = vec![DllRow {
                name: name.into(),
                mode: OverrideMode::Native,
            }];
            assert!(draft.to_game().is_err(), "expected {name:?} to fail");
        }
    }

    #[test]
    fn args_are_trimmed_and_blank_rows_skipped() {
        let mut draft = draft();
        draft.args_rows = vec!["-skip".into(), "  ".into(), " --windowed ".into()];
        assert_eq!(
            draft.to_game().unwrap().args,
            vec!["-skip".to_string(), "--windowed".to_string()]
        );
    }

    #[test]
    fn from_game_round_trips() {
        let mut game = draft().to_game().unwrap();
        game.id = Uuid::new_v4();
        game.env_overrides = BTreeMap::from([("A".into(), "1".into())]);
        game.dll_overrides = vec![DllOverride {
            dll_name: "d3d11".into(),
            mode: OverrideMode::NativeBuiltin,
        }];
        game.args = vec!["-skip".into()];
        game.display = DisplayConfig {
            resolution: Some((2560, 1440)),
            refresh_rate: Some(144),
            fullscreen: false,
            gamescope: GamescopeMode::Forced,
            ..DisplayConfig::default()
        };

        let draft = GameDraft::from_game(&game);
        assert_eq!(draft.to_game().unwrap(), game);
    }
}
