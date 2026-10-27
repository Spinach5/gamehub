use crate::{AppError, AppService};
use launcher_core::Game;

impl AppService {
    pub async fn save_game(self, game: Game, is_new: bool) -> Result<Game, AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            if !game.executable_path.is_file() {
                return Err(AppError::ExecutableMissing(game.executable_path.clone()));
            }
            if let Some(working_dir) = &game.working_dir {
                if !working_dir.is_dir() {
                    return Err(AppError::WorkingDirMissing(working_dir.clone()));
                }
            }

            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            if is_new {
                db.insert_game(&game)?;
            } else {
                db.update_game(&game)?;
            }
            Ok(game)
        })
        .await?
    }
}
