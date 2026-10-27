use crate::{AppError, AppService};
use launcher_core::Runner;
use std::path::PathBuf;

impl AppService {
    pub async fn detect_and_persist_runners(self) -> Result<Vec<Runner>, AppError> {
        let found = runner_wine::detect_system_wine().await;
        self.persist_runners(found).await
    }

    pub async fn persist_runners(self, runners: Vec<Runner>) -> Result<Vec<Runner>, AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            for mut runner in runners {
                let existing = db
                    .list_runners()?
                    .into_iter()
                    .find(|existing| existing.binary_path == runner.binary_path);

                if let Some(existing) = existing {
                    runner.id = existing.id;
                    db.update_runner(&runner)?;
                } else {
                    match db.insert_runner(&runner) {
                        Ok(()) | Err(storage::StorageError::DuplicateRunner { .. }) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            Ok(db.list_runners()?)
        })
        .await?
    }

    pub async fn import_proton(self, dir: PathBuf) -> Result<Runner, AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            let runner = runner_wine::validate_proton_dir(&dir)?;
            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            db.insert_runner(&runner)?;
            Ok(runner)
        })
        .await?
    }
}
