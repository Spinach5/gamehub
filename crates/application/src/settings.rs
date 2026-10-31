use crate::{AppError, AppService};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use uuid::Uuid;

pub const SETTING_GLOBAL_ENV: &str = "global_env";
pub const SETTING_DEFAULT_RUNNER_ID: &str = "default_runner_id";
pub const SETTING_PREFIX_ROOT: &str = "prefix_root";
pub const SETTING_ENABLE_ESYNC: &str = "enable_esync";
pub const SETTING_ENABLE_FSYNC: &str = "enable_fsync";
pub const SETTING_ENABLE_MANGOHUD: &str = "enable_mangohud";
pub const SETTING_AUTO_INSTALL_DXVK_NVAPI: &str = "auto_install_dxvk_nvapi";
pub const SETTING_DXVK_NVAPI_SOURCE: &str = "dxvk_nvapi_source";
pub const SETTING_UI_THEME: &str = "ui_theme";
pub const SETTING_UI_LANGUAGE: &str = "ui_language";
pub const SETTING_UI_FONT: &str = "ui_font";

pub const DLL_SCAN_DIR_SETTING_PREFIX: &str = "dll_scan_dir.";

pub fn dll_scan_dir_setting_key(game_id: Uuid) -> String {
    format!("{DLL_SCAN_DIR_SETTING_PREFIX}{game_id}")
}

/// Parses remembered scan directories, ignoring malformed keys and empty values.
pub fn dll_scan_dirs(settings: &BTreeMap<String, String>) -> HashMap<Uuid, PathBuf> {
    settings
        .iter()
        .filter_map(|(key, value)| {
            let raw = key.strip_prefix(DLL_SCAN_DIR_SETTING_PREFIX)?;
            let id = Uuid::parse_str(raw).ok()?;
            (!value.is_empty()).then(|| (id, PathBuf::from(value)))
        })
        .collect()
}

impl AppService {
    pub async fn save_default_runner(self, id: Option<Uuid>) -> Result<(), AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            match id {
                Some(id) => db.set_setting(SETTING_DEFAULT_RUNNER_ID, &id.to_string())?,
                None => db.delete_setting(SETTING_DEFAULT_RUNNER_ID)?,
            }
            Ok(())
        })
        .await?
    }

    pub async fn save_settings(
        self,
        values: BTreeMap<String, Option<String>>,
    ) -> Result<(), AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            for (key, value) in values {
                match value {
                    Some(value) => db.set_setting(&key, &value)?,
                    None => db.delete_setting(&key)?,
                }
            }
            Ok(())
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_dirs_parse_only_prefixed_valid_uuids() {
        let id = Uuid::new_v4();
        let settings = BTreeMap::from([
            (dll_scan_dir_setting_key(id), "/games/one".to_string()),
            ("dll_scan_dir.not-a-uuid".to_string(), "/x".to_string()),
            (dll_scan_dir_setting_key(Uuid::new_v4()), String::new()),
            ("other".to_string(), "/y".to_string()),
        ]);

        let parsed = dll_scan_dirs(&settings);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.get(&id), Some(&PathBuf::from("/games/one")));
    }
}
