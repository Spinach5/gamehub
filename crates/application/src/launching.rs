use crate::{AppError, AppService};
use launcher_core::RunnerKind;
use std::path::Path;
use tokio::sync::mpsc::UnboundedSender;
use uuid::Uuid;

pub use ::launch::{LaunchRequest, LaunchTuning, validate_dll_name, validate_env_key};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchOutcome {
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stopped: bool,
    pub session_seconds: i64,
}

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Started {
        game_id: Uuid,
        started_at: i64,
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

        let plan_request = request.clone();
        let plan =
            tokio::task::spawn_blocking(move || ::launch::build_plan(&plan_request)).await??;

        let target_monitor = request
            .game
            .display
            .monitor
            .as_deref()
            .map(str::trim)
            .filter(|monitor| !monitor.is_empty())
            .map(str::to_string);
        let uses_gamescope = display::should_use_gamescope(
            &request.game.display,
            request.session,
            request.gamescope_path.is_some(),
        );
        let primary_to_restore = if !uses_gamescope && plan.env.contains_key("DISPLAY") {
            match target_monitor {
                Some(monitor) => {
                    let session = request.session;
                    match tokio::task::spawn_blocking(move || {
                        display::set_primary_output(&monitor, session)
                    })
                    .await
                    {
                        Ok(Ok(previous)) => previous,
                        Ok(Err(error)) => {
                            tracing::warn!(%error, "cannot switch primary monitor");
                            None
                        }
                        Err(error) => {
                            tracing::warn!(%error, "primary monitor task failed");
                            None
                        }
                    }
                }
                None => None,
            }
        } else {
            None
        };

        let mut child =
            match ::launch::spawn_plan_with_log(&plan, request.log_path.as_deref()).await {
                Ok(child) => child,
                Err(error) => {
                    restore_primary(primary_to_restore, request.session).await;
                    return Err(error.into());
                }
            };
        let process_group = child.id();
        if let Some(process_group) = process_group {
            if !self.register_process(
                game_id,
                process_group,
                request.runner.clone(),
                wine_prefix.clone(),
            ) {
                let _ = child.kill().await;
                restore_primary(primary_to_restore, request.session).await;
                return Err(AppError::GameAlreadyRunning);
            }
        }
        // Record the start before the game can report back, so a launcher
        // crash mid-session still leaves an accurate "last played" stamp.
        let started_at = unix_now();
        let db = self.db_handle();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(db) = db.lock() {
                let _ = db.touch_last_played(game_id, started_at);
            }
        })
        .await;
        let _ = events.send(LaunchEvent::Started {
            game_id,
            started_at,
        });

        // Wine reads the XRandR primary while starting up and keeps using it
        // for the game window, so the original primary can be restored early.
        if primary_to_restore.is_some() {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            restore_primary(primary_to_restore.clone(), request.session).await;
        }

        let wait_result = child.wait().await;
        restore_primary(primary_to_restore, request.session).await;
        if let Some(process_group) = process_group {
            wait_for_session_end(process_group, &wine_prefix).await;
        }
        self.unregister_process(game_id);
        let stopped = self.take_stop_requested(game_id);
        let session_seconds = unix_now().saturating_sub(started_at);

        let db = self.db_handle();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(db) = db.lock() {
                let _ = db.add_playtime(game_id, session_seconds);
            }
        })
        .await;

        let status = wait_result.map_err(AppError::ProcessWait)?;
        let outcome = LaunchOutcome {
            exit_code: status.code(),
            success: status.success(),
            stopped,
            session_seconds,
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

/// The wrapper we spawned can exit while the game keeps running: Wine starts
/// game processes from wineserver, and launchers such as `proton run` may
/// return before the game does. Keep the session open until both our process
/// group and the prefix are empty, so the UI does not report a live game as
/// idle and the stop button keeps working.
async fn wait_for_session_end(process_group: u32, wine_prefix: &Path) {
    let mut announced = false;

    loop {
        let prefix = wine_prefix.to_path_buf();
        let alive = tokio::task::spawn_blocking(move || {
            ::launch::process_group_alive(process_group)
                || runner_wine::prefix_has_processes(&prefix)
        })
        .await
        .unwrap_or(false);

        if !alive {
            return;
        }
        if !announced {
            announced = true;
            tracing::debug!(
                process_group,
                prefix = %wine_prefix.display(),
                "launcher wrapper exited while the game is still running"
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

async fn restore_primary(previous: Option<String>, session: display::SessionType) {
    let Some(previous) = previous else {
        return;
    };
    match tokio::task::spawn_blocking(move || display::set_primary_output(&previous, session)).await
    {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(%error, "failed to restore primary monitor"),
        Err(error) => tracing::warn!(%error, "primary monitor restore task failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A group id that no live process can have.
    const DEAD_GROUP: u32 = 2_000_000_000;

    #[tokio::test]
    async fn session_ends_immediately_when_nothing_is_left() {
        let dir = tempfile::tempdir().unwrap();
        tokio::time::timeout(
            Duration::from_secs(2),
            wait_for_session_end(DEAD_GROUP, &dir.path().join("pfx")),
        )
        .await
        .expect("an idle prefix ends the session right away");
    }

    #[tokio::test]
    async fn session_stays_open_while_the_prefix_has_processes() {
        let dir = tempfile::tempdir().unwrap();
        let wine_prefix = dir.path().join("pfx");
        let mut child = tokio::process::Command::new("sleep")
            .arg("30")
            .env("WINEPREFIX", &wine_prefix)
            .spawn()
            .unwrap();

        for _ in 0..100 {
            if runner_wine::prefix_has_processes(&wine_prefix) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(runner_wine::prefix_has_processes(&wine_prefix));

        let mut waiter = tokio::spawn({
            let wine_prefix = wine_prefix.clone();
            async move { wait_for_session_end(DEAD_GROUP, &wine_prefix).await }
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(700), &mut waiter)
                .await
                .is_err(),
            "the session must stay open while a detached game process lives"
        );

        let _ = child.kill().await;
        let _ = child.wait().await;

        tokio::time::timeout(Duration::from_secs(3), waiter)
            .await
            .expect("the session ends once the prefix is empty")
            .expect("session waiter task finishes");
    }
}
