use crate::{AppError, AppService};
use launcher_core::RunnerKind;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

pub use ::launch::{LaunchRequest, LaunchTuning, validate_dll_name, validate_env_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stopped: bool,
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

        if self.running_game(game_id).is_some() {
            return Err(AppError::GameAlreadyRunning);
        }

        let prefix_plan = ::prefix::plan_for_game(
            &request.game,
            request.runner.clone(),
            &request.default_prefix_root,
            request.steam_compat_client.clone(),
        );

        if request.tuning.enable_dxvk_nvapi && request.runner.kind == RunnerKind::Wine {
            let source = request
                .dxvk_nvapi_source
                .as_ref()
                .ok_or(AppError::DxvkNvapiSourceMissing)?;
            ::prefix::install_dxvk_nvapi(
                &prefix_plan,
                &::prefix::DxvkNvapiSource {
                    root: source.clone(),
                },
            )
            .await?;
        }

        let wine_prefix = ::prefix::windows_root(&prefix_plan.path, request.runner.kind);
        let runner = request.runner.clone();

        let plan_request = request.clone();
        let plan =
            tokio::task::spawn_blocking(move || ::launch::build_plan(&plan_request)).await??;

        let mut child = ::launch::spawn_plan_with_log(&plan, request.log_path.as_deref()).await?;
        if let Some(process_group) = child.id() {
            if !self.register_process(game_id, process_group, runner, wine_prefix) {
                let _ = child.kill().await;
                return Err(AppError::GameAlreadyRunning);
            }
        }
        let _ = events.send(LaunchEvent::Started { game_id });

        let wait_result = child.wait().await;
        self.unregister_process(game_id);
        let stopped = self.take_stop_requested(game_id);
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
            stopped,
        };
        let _ = events.send(LaunchEvent::Finished { game_id, outcome });
        Ok(outcome)
    }

    pub async fn stop_game(self, game_id: Uuid) -> Result<(), AppError> {
        let Some(running) = self.running_game(game_id) else {
            return Ok(());
        };
        self.mark_stop_requested(game_id);

        // Wine spawns game processes from wineserver, which detaches them from
        // the launcher's process group; stop the whole prefix first.
        let prefix_result = runner_wine::kill_wine_prefix(&running.runner, &running.wine_prefix)
            .await
            .map_err(AppError::Runner);
        let group_result = ::launch::terminate_process_group(running.process_group)
            .await
            .map_err(AppError::Stop);

        match (prefix_result, group_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(error)) => {
                tracing::warn!(%error, "process group termination failed after wineserver shutdown");
                Ok(())
            }
            (Err(error), Ok(())) => {
                tracing::warn!(%error, "wineserver shutdown failed after process group termination");
                Ok(())
            }
            (Err(_), Err(error)) => {
                self.clear_stop_requested(game_id);
                Err(error)
            }
        }
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
