use crate::{AppError, AppService};
use launcher_core::RunnerKind;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

pub use ::launch::{LaunchRequest, LaunchTuning, validate_dll_name, validate_env_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Started {
        game_id: Uuid,
    },
    Finished {
        game_id: Uuid,
        outcome: LaunchOutcome,
    },
    Failed {
        game_id: Uuid,
        error: String,
    },
}

impl AppService {
    pub async fn launch_game(self, request: LaunchRequest) -> Result<LaunchOutcome, AppError> {
        let (events, _receiver) = tokio::sync::mpsc::unbounded_channel();
        self.launch_game_with_events(request, events).await
    }

    pub async fn launch_game_with_events(
        self,
        request: LaunchRequest,
        events: UnboundedSender<LaunchEvent>,
    ) -> Result<LaunchOutcome, AppError> {
        let game_id = request.game.id;
        let result = self.run_launch(request, &events).await;
        if let Err(error) = &result {
            let _ = events.send(LaunchEvent::Failed {
                game_id,
                error: error.to_string(),
            });
        }
        result
    }

    async fn run_launch(
        &self,
        request: LaunchRequest,
        events: &UnboundedSender<LaunchEvent>,
    ) -> Result<LaunchOutcome, AppError> {
        let game_id = request.game.id;

        if request.tuning.enable_dxvk_nvapi && request.runner.kind == RunnerKind::Wine {
            let source = request
                .dxvk_nvapi_source
                .as_ref()
                .ok_or(AppError::DxvkNvapiSourceMissing)?;
            let prefix_plan = ::prefix::plan_for_game(
                &request.game,
                request.runner.clone(),
                &request.default_prefix_root,
                request.steam_compat_client.clone(),
            );
            ::prefix::install_dxvk_nvapi(
                &prefix_plan,
                &::prefix::DxvkNvapiSource {
                    root: source.clone(),
                },
            )
            .await?;
        }

        let plan_request = request.clone();
        let plan =
            tokio::task::spawn_blocking(move || ::launch::build_plan(&plan_request)).await??;

        let mut child = ::launch::spawn_plan_with_log(&plan, request.log_path.as_deref()).await?;
        let _ = events.send(LaunchEvent::Started { game_id });

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
        let outcome = LaunchOutcome {
            exit_code: status.code(),
            success: status.success(),
        };
        let _ = events.send(LaunchEvent::Finished { game_id, outcome });
        Ok(outcome)
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
