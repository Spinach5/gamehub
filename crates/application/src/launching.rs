use crate::{AppError, AppService};

pub use ::launch::{LaunchRequest, validate_dll_name, validate_env_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
}

impl AppService {
    pub async fn launch_game(self, request: LaunchRequest) -> Result<LaunchOutcome, AppError> {
        let game_id = request.game.id;
        let plan_request = request.clone();

        let plan =
            tokio::task::spawn_blocking(move || ::launch::build_plan(&plan_request)).await??;

        let mut child = ::launch::spawn_plan(&plan).await?;
        let wait_result = child.wait().await;
        let timestamp = unix_now();

        let db = self.db_handle();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(db) = db.lock() {
                let _ = db.touch_last_played(game_id, timestamp);
            }
        })
        .await;

        let status = wait_result.map_err(AppError::ProcessWait)?;
        Ok(LaunchOutcome {
            exit_code: status.code(),
            success: status.success(),
        })
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
