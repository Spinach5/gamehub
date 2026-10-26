use crate::{LaunchError, LaunchPlan};
use std::process::Stdio;
use tokio::process::{Child, Command};

pub async fn spawn_plan(plan: &LaunchPlan) -> Result<Child, LaunchError> {
    let mut command = Command::new(&plan.program);
    command
        .args(&plan.args)
        .env_clear()
        .envs(&plan.env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(false);

    if let Some(cwd) = &plan.cwd {
        command.current_dir(cwd);
    }

    command.spawn().map_err(|source| LaunchError::Spawn {
        program: plan.program.clone(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
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
}
