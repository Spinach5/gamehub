use crate::{AppError, AppService};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteTarget {
    Game(Uuid),
    Runner(Uuid),
    Prefix { game_id: Uuid, path: PathBuf },
}

impl DeleteTarget {
    pub fn label(&self) -> String {
        match self {
            Self::Game(id) => format!("游戏 {id}"),
            Self::Runner(id) => format!("Runner {id}"),
            Self::Prefix { path, .. } => format!("Prefix {}", path.display()),
        }
    }
}

impl AppService {
    pub async fn delete(
        self,
        target: DeleteTarget,
        running: HashMap<Uuid, PathBuf>,
        default_runner_id: Option<Uuid>,
        logs_dir: PathBuf,
    ) -> Result<(), AppError> {
        match target {
            DeleteTarget::Runner(id) => {
                let db = self.db_handle();
                tokio::task::spawn_blocking(move || {
                    if default_runner_id == Some(id) {
                        return Err(AppError::DefaultRunnerInUse);
                    }
                    let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
                    let count = db.count_games_using_runner(id)?;
                    if count > 0 {
                        return Err(AppError::RunnerInUse(count));
                    }
                    db.delete_runner(id)?;
                    Ok(())
                })
                .await?
            }
            DeleteTarget::Prefix { game_id, path } => {
                if running.contains_key(&game_id)
                    || running.values().any(|running_path| running_path == &path)
                {
                    return Err(AppError::PrefixInUseByRunningGame);
                }

                let path_for_db = path.clone();
                let db = self.db_handle();
                tokio::task::spawn_blocking(move || {
                    let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
                    let count = db.count_games_using_prefix(&path_for_db)?;
                    if count > 1 {
                        return Err(AppError::PrefixShared);
                    }
                    if count == 1 {
                        let mut game = db.get_game(game_id)?.ok_or(AppError::GameNotFound)?;
                        if game.prefix_path.as_deref() != Some(path_for_db.as_path()) {
                            return Err(AppError::PrefixUsedByOtherGame);
                        }
                        game.prefix_path = None;
                        db.update_game(&game)?;
                    }
                    Ok(())
                })
                .await??;

                ::prefix::delete_prefix(&path).await?;
                Ok(())
            }
            DeleteTarget::Game(id) => {
                if running.contains_key(&id) {
                    return Err(AppError::GameRunning);
                }
                let log_path = logs_dir.join("games").join(format!("{id}.log"));
                let db = self.db_handle();
                tokio::task::spawn_blocking(move || {
                    let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
                    let cover = db.get_game(id)?.and_then(|game| game.cover_path);
                    db.delete_game(id)?;
                    if let Some(path) = cover {
                        if let Err(error) = std::fs::remove_file(&path) {
                            tracing::warn!(
                                error = %error,
                                path = %path.display(),
                                "failed to remove game cover"
                            );
                        }
                    }
                    if let Err(error) = std::fs::remove_file(&log_path) {
                        if error.kind() != std::io::ErrorKind::NotFound {
                            tracing::warn!(
                                error = %error,
                                path = %log_path.display(),
                                "failed to remove game log"
                            );
                        }
                    }
                    Ok(())
                })
                .await?
            }
        }
    }
}
