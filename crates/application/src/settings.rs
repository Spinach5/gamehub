use crate::{AppError, AppService};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const SETTING_GLOBAL_ENV: &str = "global_env";
pub const SETTING_DEFAULT_RUNNER_ID: &str = "default_runner_id";
pub const SETTING_PREFIX_ROOT: &str = "prefix_root";

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
