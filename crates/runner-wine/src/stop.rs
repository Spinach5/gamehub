use crate::RunnerError;
use launcher_core::{Runner, RunnerKind};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub fn wineserver_path(runner: &Runner) -> Option<PathBuf> {
    let candidates = match runner.kind {
        RunnerKind::Wine => vec![runner.binary_path.with_file_name("wineserver")],
        RunnerKind::Proton => vec![
            runner.root_dir.join("files/bin/wineserver"),
            runner.root_dir.join("dist/bin/wineserver"),
            runner.root_dir.join("bin/wineserver"),
        ],
    };

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .or_else(|| find_in_path("wineserver"))
}

/// Stops every process owned by the prefix's wineserver, including game
/// processes that were detached from the launcher's process group.
pub async fn kill_wine_prefix(runner: &Runner, wine_prefix: &Path) -> Result<(), RunnerError> {
    let Some(wineserver) = wineserver_path(runner) else {
        tracing::debug!(
            prefix = %wine_prefix.display(),
            "no wineserver found; relying on process-group termination"
        );
        return Ok(());
    };

    let output = Command::new(&wineserver)
        .arg("-k")
        .env("WINEPREFIX", wine_prefix)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|source| RunnerError::Spawn {
            program: wineserver,
            source,
        })?;

    if !output.status.success() {
        tracing::debug!(
            code = ?output.status.code(),
            prefix = %wine_prefix.display(),
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "wineserver -k found no running server"
        );
    }

    Ok(())
}

fn find_in_path(program: &str) -> Option<PathBuf> {
    find_in_path_with(program, std::env::var_os("PATH").as_deref())
}

fn find_in_path_with(program: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    let path_env = path_env?;
    std::env::split_paths(path_env)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, RunnerSource};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn runner(kind: RunnerKind, binary: PathBuf, root: PathBuf) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind,
            name: "test".into(),
            binary_path: binary,
            root_dir: root,
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    #[test]
    fn wine_uses_the_sibling_wineserver() {
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        let wineserver = dir.path().join("wineserver");
        fs::write(&wine, b"").unwrap();
        fs::write(&wineserver, b"").unwrap();

        let runner = runner(RunnerKind::Wine, wine, dir.path().to_path_buf());
        assert_eq!(wineserver_path(&runner), Some(wineserver));
    }

    #[test]
    fn proton_prefers_the_bundled_wineserver() {
        let dir = tempdir().unwrap();
        let bin = dir.path().join("files/bin");
        fs::create_dir_all(&bin).unwrap();
        let wineserver = bin.join("wineserver");
        fs::write(&wineserver, b"").unwrap();
        let proton = dir.path().join("proton");
        fs::write(&proton, b"").unwrap();

        let runner = runner(RunnerKind::Proton, proton, dir.path().to_path_buf());
        assert_eq!(wineserver_path(&runner), Some(wineserver));
    }

    #[tokio::test]
    async fn kill_prefix_invokes_wineserver_with_the_prefix() {
        let _guard = crate::test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        let wineserver = dir.path().join("wineserver");
        let output = dir.path().join("args.txt");

        fs::write(&wine, b"").unwrap();
        fs::write(
            &wineserver,
            format!(
                "#!/bin/sh\nprintf '%s|%s' \"$1\" \"$WINEPREFIX\" > '{}'\n",
                output.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&wineserver).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&wineserver, permissions).unwrap();

        let runner = runner(RunnerKind::Wine, wine, dir.path().to_path_buf());
        kill_wine_prefix(&runner, Path::new("/data/prefix"))
            .await
            .unwrap();

        assert_eq!(fs::read_to_string(output).unwrap(), "-k|/data/prefix");
    }
}
