use crate::{AppError, AppService};
use launcher_core::Game;
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_COVER_EDGE: u32 = 8192;
const COVER_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

fn import_cover(
    source: &Path,
    game_id: Uuid,
    covers_dir: &Path,
) -> Result<(PathBuf, Option<PathBuf>), AppError> {
    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .ok_or_else(|| AppError::CoverUnsupportedFormat(source.to_path_buf()))?;
    if !COVER_EXTENSIONS.contains(&extension.as_str()) {
        return Err(AppError::CoverUnsupportedFormat(source.to_path_buf()));
    }

    let invalid = |detail: String| AppError::CoverInvalidImage {
        path: source.to_path_buf(),
        detail,
    };

    let reader = image::io::Reader::open(source)
        .map_err(|error| invalid(error.to_string()))?
        .with_guessed_format()
        .map_err(|error| invalid(error.to_string()))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| invalid(error.to_string()))?;
    if width == 0 || height == 0 {
        return Err(invalid("图片尺寸为 0".into()));
    }
    if width > MAX_COVER_EDGE || height > MAX_COVER_EDGE {
        return Err(AppError::CoverTooLarge { width, height });
    }

    std::fs::create_dir_all(covers_dir).map_err(AppError::CoverCopy)?;
    let target = covers_dir.join(format!("{game_id}.{extension}"));
    let temp = covers_dir.join(format!("{game_id}.{extension}.tmp"));
    if let Err(error) = std::fs::copy(source, &temp) {
        let _ = std::fs::remove_file(&temp);
        return Err(AppError::CoverCopy(error));
    }

    // Move an existing target aside so a failed save can put it back instead
    // of leaving the database pointing at replaced content.
    let backup = if target.is_file() {
        let backup = covers_dir.join(format!("{game_id}.{extension}.bak"));
        if let Err(error) = std::fs::rename(&target, &backup) {
            let _ = std::fs::remove_file(&temp);
            return Err(AppError::CoverCopy(error));
        }
        Some(backup)
    } else {
        None
    };

    if let Err(error) = std::fs::rename(&temp, &target) {
        let _ = std::fs::remove_file(&temp);
        if let Some(backup) = &backup {
            let _ = std::fs::rename(backup, &target);
        }
        return Err(AppError::CoverCopy(error));
    }
    Ok((target, backup))
}

impl AppService {
    pub async fn save_game(
        self,
        game: Game,
        is_new: bool,
        cover_source: Option<PathBuf>,
        covers_dir: PathBuf,
    ) -> Result<Game, AppError> {
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
            let previous_cover = db.get_game(game.id)?.and_then(|stored| stored.cover_path);

            let mut game = game;
            let mut imported_cover = None;
            let mut imported_backup = None;
            if let Some(source) = cover_source {
                let (target, backup) = import_cover(&source, game.id, &covers_dir)?;
                game.cover_path = Some(target.clone());
                imported_cover = Some(target);
                imported_backup = backup;
            }

            let result = if is_new {
                db.insert_game(&game)
            } else {
                db.update_game(&game)
            };
            if let Err(error) = result {
                if let Some(path) = &imported_cover {
                    if let Some(backup) = &imported_backup {
                        if let Err(restore_error) = std::fs::rename(backup, path) {
                            tracing::warn!(
                                error = %restore_error,
                                backup = %backup.display(),
                                path = %path.display(),
                                "failed to restore previous cover after save failure"
                            );
                        }
                    } else if previous_cover.as_deref() != Some(path.as_path()) {
                        if let Err(remove_error) = std::fs::remove_file(path) {
                            tracing::warn!(
                                error = %remove_error,
                                path = %path.display(),
                                "failed to remove imported cover after save failure"
                            );
                        }
                    }
                }
                return Err(error.into());
            }

            if let Some(backup) = imported_backup {
                if let Err(error) = std::fs::remove_file(&backup) {
                    tracing::warn!(
                        error = %error,
                        path = %backup.display(),
                        "failed to remove cover backup"
                    );
                }
            }

            if let Some(previous) = previous_cover {
                if game.cover_path.as_deref() != Some(previous.as_path()) {
                    if let Err(error) = std::fs::remove_file(&previous) {
                        tracing::warn!(
                            error = %error,
                            path = %previous.display(),
                            "failed to remove old cover"
                        );
                    }
                }
            }

            Ok(game)
        })
        .await?
    }
}
