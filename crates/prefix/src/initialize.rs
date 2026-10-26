use crate::{PrefixError, PrefixPlan, PrefixStatus, status};
use launcher_core::truncate_utf8;
use runner_wine::{RunnerError, WineContext};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const WINEBOOT_TIMEOUT: Duration = Duration::from_secs(120);

impl PrefixPlan {
    pub async fn initialize(&self) -> Result<bool, PrefixError> {
        self.initialize_with_timeout(WINEBOOT_TIMEOUT).await
    }

    pub async fn initialize_with_timeout(&self, timeout: Duration) -> Result<bool, PrefixError> {
        match status(self) {
            PrefixStatus::Initialized => return Ok(false),
            PrefixStatus::ArchMismatch { existing } => {
                return Err(PrefixError::ArchMismatch {
                    expected: self.arch,
                    existing,
                });
            }
            PrefixStatus::Missing | PrefixStatus::NotInitialized => {}
        }

        tokio::fs::create_dir_all(&self.path).await?;
        tokio::fs::create_dir_all(&self.steam_compat_client).await?;

        let context = WineContext {
            runner: self.runner.clone(),
            prefix: self.path.clone(),
            arch: self.arch,
            steam_compat_client: self.steam_compat_client.clone(),
        };

        let (program, args) = context.build_command(Path::new("wineboot"), &["-u".to_string()]);
        let mut command = Command::new(&program);
        command
            .args(&args)
            .envs(context.base_env())
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command
            .spawn()
            .map_err(|source| PrefixError::Runner(RunnerError::Spawn { program, source }))?;

        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| PrefixError::WinebootTimeout {
                seconds: timeout.as_secs(),
            })??;

        if !output.status.success() {
            return Err(PrefixError::WinebootFailed {
                code: output.status.code(),
                stderr: truncate_utf8(&String::from_utf8_lossy(&output.stderr), 4096),
            });
        }

        match status(self) {
            PrefixStatus::Initialized => Ok(true),
            _ => Err(PrefixError::InitializationIncomplete),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{EXEC_LOCK, fake_runner, write_script};
    use launcher_core::{Arch, RunnerKind};
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::tempdir;

    #[tokio::test]
    async fn wineboot_creates_system_reg_and_skips_second_run() {
        let _exec_guard = EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let script = dir.path().join("wine");
        write_script(
            &script,
            r#"#!/bin/sh
mkdir -p "$WINEPREFIX"
printf '#arch=win64\n' > "$WINEPREFIX/system.reg"
"#,
        );

        let prefix = dir.path().join("prefix");
        let plan = PrefixPlan::new(
            prefix.clone(),
            fake_runner(RunnerKind::Wine, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(
            plan.initialize_with_timeout(Duration::from_secs(2))
                .await
                .unwrap()
        );
        assert!(prefix.join("system.reg").is_file());
        assert!(
            !plan
                .initialize_with_timeout(Duration::from_secs(2))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn wineboot_nonzero_exit_reports_stderr() {
        let _exec_guard = EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let script = dir.path().join("wine");
        write_script(&script, "#!/bin/sh\necho 'boom' >&2\nexit 3\n");

        let plan = PrefixPlan::new(
            dir.path().join("prefix"),
            fake_runner(RunnerKind::Wine, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        match plan.initialize_with_timeout(Duration::from_secs(2)).await {
            Err(PrefixError::WinebootFailed {
                code: Some(3),
                stderr,
            }) => assert!(stderr.contains("boom")),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn wineboot_timeout_kills_the_child() {
        let _exec_guard = EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let script = dir.path().join("wine");
        write_script(&script, "#!/bin/sh\nsleep 5\n");

        let plan = PrefixPlan::new(
            dir.path().join("prefix"),
            fake_runner(RunnerKind::Wine, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        let started = std::time::Instant::now();
        let result = plan
            .initialize_with_timeout(Duration::from_millis(50))
            .await;
        assert!(matches!(result, Err(PrefixError::WinebootTimeout { .. })));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn arch_mismatch_is_rejected_before_spawning() {
        let dir = tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), "#arch=win32\n").unwrap();

        let plan = PrefixPlan::new(
            prefix,
            fake_runner(RunnerKind::Wine, PathBuf::from("/does/not/exist")),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(matches!(
            plan.initialize_with_timeout(Duration::from_secs(2)).await,
            Err(PrefixError::ArchMismatch {
                existing: Some(Arch::Win32),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn proton_initialization_uses_pfx_layout_and_run_subcommand() {
        let _exec_guard = EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let script = dir.path().join("proton");
        let args_log = dir.path().join("args.log");
        write_script(
            &script,
            &format!(
                r#"#!/bin/sh
printf '%s\n' "$@" > '{}'
mkdir -p "$STEAM_COMPAT_DATA_PATH/pfx"
printf '#arch=win64\n' > "$STEAM_COMPAT_DATA_PATH/pfx/system.reg"
"#,
                args_log.display()
            ),
        );

        let prefix = dir.path().join("compat-data");
        let plan = PrefixPlan::new(
            prefix.clone(),
            fake_runner(RunnerKind::Proton, script),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(
            plan.initialize_with_timeout(Duration::from_secs(2))
                .await
                .unwrap()
        );
        assert!(prefix.join("pfx/system.reg").is_file());
        assert!(!prefix.join("system.reg").exists());

        let logged = fs::read_to_string(args_log).unwrap();
        let args: Vec<&str> = logged.lines().collect();
        assert_eq!(args[0], "run");
        assert_eq!(args[1], "wineboot");
        assert_eq!(args[2], "-u");
    }

    #[tokio::test]
    #[ignore = "requires a working system wine; run manually"]
    async fn initializes_with_real_system_wine() {
        let runners = runner_wine::detect_system_wine().await;
        let runner = runners.into_iter().next().expect("no system wine found");

        let dir = tempdir().unwrap();
        let plan = PrefixPlan::new(
            dir.path().join("prefix"),
            runner,
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(
            plan.initialize_with_timeout(Duration::from_secs(180))
                .await
                .unwrap()
        );
        assert!(plan.path.join("system.reg").is_file());
    }
}
