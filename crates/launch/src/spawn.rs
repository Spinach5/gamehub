use crate::{LaunchError, LaunchPlan};
use nix::errno::Errno;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use std::fs::OpenOptions as StdOpenOptions;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub async fn spawn_plan(plan: &LaunchPlan) -> Result<Child, LaunchError> {
    spawn_plan_with_log(plan, None).await
}

pub async fn spawn_plan_with_log(
    plan: &LaunchPlan,
    log_path: Option<&Path>,
) -> Result<Child, LaunchError> {
    let log = match log_path {
        Some(path) => match open_log(path, plan).await {
            Ok(file) => Some(Arc::new(Mutex::new(file))),
            Err(error) => {
                tracing::warn!(
                    %error,
                    path = %path.display(),
                    "failed to open launch log; falling back to inherited stderr"
                );
                None
            }
        },
        None => None,
    };

    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .env_clear()
        .envs(&plan.env)
        .stdin(Stdio::null())
        .process_group(0)
        .kill_on_drop(false);

    if log.is_some() {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::null()).stderr(Stdio::inherit());
    }

    if let Some(cwd) = &plan.cwd {
        command.current_dir(cwd);
    }

    let mut child = command.spawn().map_err(|source| LaunchError::Spawn {
        program: plan.program.clone(),
        source,
    })?;

    if let Some(log) = log {
        if let Some(stdout) = child.stdout.take() {
            spawn_reader(stdout, log.clone(), false);
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_reader(stderr, log, true);
        }
    }

    Ok(child)
}

/// Terminates the whole process group, falling back to SIGKILL when the group
/// does not exit shortly after SIGTERM.
pub async fn terminate_process_group(process_group: u32) -> Result<(), LaunchError> {
    let group = Pid::from_raw(process_group as i32);
    match killpg(group, Signal::SIGTERM) {
        Ok(()) | Err(Errno::ESRCH) => {}
        Err(error) => return Err(LaunchError::Terminate(error.to_string())),
    }

    for _ in 0..50 {
        match killpg(group, None::<Signal>) {
            Ok(()) => tokio::time::sleep(Duration::from_millis(100)).await,
            Err(Errno::ESRCH) => return Ok(()),
            Err(error) => return Err(LaunchError::Terminate(error.to_string())),
        }
    }

    match killpg(group, Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => Err(LaunchError::Terminate(error.to_string())),
    }
}

async fn open_log(path: &Path, plan: &LaunchPlan) -> std::io::Result<File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        tokio::fs::create_dir_all(parent).await?;
    }

    if tokio::fs::try_exists(path).await? {
        let mut old = path.as_os_str().to_owned();
        old.push(".old");
        let old = PathBuf::from(old);
        match tokio::fs::remove_file(&old).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        tokio::fs::rename(path, &old).await?;
    }

    let mut file = StdOpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    let command_line = std::iter::once(plan.program.to_string_lossy().into_owned())
        .chain(plan.args.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    let header = format!(
        "===== gamehub launch {} =====\nRunning command: {command_line}\n",
        chrono::Utc::now().to_rfc3339()
    );
    std::io::Write::write_all(&mut file, header.as_bytes())?;
    Ok(File::from_std(file))
}

fn spawn_reader<R>(reader: R, log: Arc<Mutex<File>>, stderr: bool)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut file = log.lock().await;
            let written = if stderr {
                file.write_all(format!("[stderr] {line}\n").as_bytes())
                    .await
            } else {
                file.write_all(format!("{line}\n").as_bytes()).await
            };
            if written.is_err() {
                break;
            }
            let _ = file.flush().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn clears_environment_and_returns_child_status() {
        let dir = tempdir().unwrap();
        let output = dir.path().join("out.txt");
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                "-c".into(),
                format!("printf '%s|%s' \"$FOO\" \"$HOME\" > '{}'", output.display()),
            ],
            env: BTreeMap::from([("FOO".into(), "bar".into())]),
            cwd: None,
        };

        let mut child = spawn_plan(&plan).await.unwrap();
        let status = child.wait().await.unwrap();
        assert!(status.success());
        assert_eq!(fs::read_to_string(output).unwrap(), "bar|");
    }

    #[tokio::test]
    async fn propagates_exit_code() {
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), "exit 7".into()],
            env: BTreeMap::new(),
            cwd: None,
        };
        let mut child = spawn_plan(&plan).await.unwrap();
        assert_eq!(child.wait().await.unwrap().code(), Some(7));
    }

    #[tokio::test]
    async fn missing_program_is_a_spawn_error() {
        let plan = LaunchPlan {
            program: PathBuf::from("/definitely/not/here"),
            args: Vec::new(),
            env: BTreeMap::new(),
            cwd: None,
        };
        assert!(matches!(
            spawn_plan(&plan).await,
            Err(LaunchError::Spawn { .. })
        ));
    }

    async fn read_log_until(path: &std::path::Path, needle: &str) -> String {
        for _ in 0..200 {
            if let Ok(text) = fs::read_to_string(path) {
                if text.contains(needle) {
                    return text;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        panic!("log {path:?} never contained {needle:?}");
    }

    #[tokio::test]
    async fn log_capture_writes_header_and_streams_both_pipes() {
        let dir = tempdir().unwrap();
        let log = dir.path().join("game.log");
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), "echo stdout-line; echo stderr-line >&2".into()],
            env: BTreeMap::new(),
            cwd: None,
        };

        let mut child = spawn_plan_with_log(&plan, Some(&log)).await.unwrap();
        assert!(child.wait().await.unwrap().success());

        // stdout and stderr have independent reader tasks, so wait for each.
        read_log_until(&log, "\nstdout-line\n").await;
        let text = read_log_until(&log, "[stderr] stderr-line").await;
        assert!(text.starts_with("===== gamehub launch "), "{text}");
        assert!(
            text.contains("Running command: /bin/sh -c echo stdout-line;"),
            "{text}"
        );
        assert!(text.contains("stdout-line"), "{text}");
        assert!(text.contains("[stderr] stderr-line"), "{text}");
    }

    #[tokio::test]
    async fn previous_log_is_rotated_to_old() {
        let dir = tempdir().unwrap();
        let log = dir.path().join("game.log");
        fs::write(&log, "stale-run\n").unwrap();
        let old = dir.path().join("game.log.old");
        fs::write(&old, "older-run\n").unwrap();

        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), "echo fresh-run".into()],
            env: BTreeMap::new(),
            cwd: None,
        };
        let mut child = spawn_plan_with_log(&plan, Some(&log)).await.unwrap();
        assert!(child.wait().await.unwrap().success());

        read_log_until(&log, "fresh-run").await;
        assert_eq!(fs::read_to_string(&old).unwrap(), "stale-run\n");
        assert!(!fs::read_to_string(&log).unwrap().contains("stale-run"));
    }

    #[tokio::test]
    async fn unwritable_log_path_does_not_block_the_launch() {
        let dir = tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        fs::write(&blocker, b"not a directory").unwrap();
        let log = blocker.join("game.log");
        let output = dir.path().join("out.txt");
        let plan = LaunchPlan {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), format!("printf ok > '{}'", output.display())],
            env: BTreeMap::new(),
            cwd: None,
        };

        let mut child = spawn_plan_with_log(&plan, Some(&log)).await.unwrap();
        assert!(child.wait().await.unwrap().success());
        assert_eq!(fs::read_to_string(output).unwrap(), "ok");
        assert!(!log.exists());
    }
}
