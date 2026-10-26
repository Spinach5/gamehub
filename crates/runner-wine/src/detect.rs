use launcher_core::{Arch, Runner, RunnerKind, RunnerSource};
use std::ffi::OsStr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use uuid::Uuid;

use crate::RunnerError;

const COMMON_WINE_PATHS: &[&str] = &[
    "/usr/bin/wine",
    "/usr/bin/wine-stable",
    "/usr/bin/wine-development",
];

fn is_executable_file(path: &Path) -> bool {
    match std::fs::metadata(path) {
        Ok(metadata) => metadata.is_file() && metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

pub fn candidate_wine_paths(path_env: Option<&OsStr>) -> Vec<PathBuf> {
    let common_paths: Vec<PathBuf> = COMMON_WINE_PATHS.iter().map(PathBuf::from).collect();
    candidate_wine_paths_with(path_env, &common_paths)
}

pub fn candidate_wine_paths_with(
    path_env: Option<&OsStr>,
    common_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path_env) = path_env {
        for dir in std::env::split_paths(path_env) {
            let candidate = dir.join("wine");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }

    for candidate in common_paths {
        if candidate.is_file() {
            candidates.push(candidate.clone());
        }
    }

    let mut seen = std::collections::HashSet::new();
    candidates.retain(|candidate| {
        let key = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.clone());
        seen.insert(key)
    });
    candidates
}

pub fn validate_proton_dir(root: &Path) -> Result<Runner, RunnerError> {
    let canonical = root
        .canonicalize()
        .map_err(|source| RunnerError::Canonicalize {
            path: root.to_path_buf(),
            source,
        })?;
    let proton = canonical.join("proton");
    if !is_executable_file(&proton) {
        return Err(RunnerError::InvalidProtonDir(root.to_path_buf()));
    }

    let version_string = std::fs::read_to_string(canonical.join("version"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            canonical
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
        });

    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Proton")
        .to_string();

    Ok(Runner {
        id: Uuid::new_v4(),
        kind: RunnerKind::Proton,
        name,
        binary_path: proton,
        root_dir: canonical,
        arch: Arch::Win64,
        source: RunnerSource::Imported {
            original_path: root.to_path_buf(),
        },
        verified: true,
        version_string,
    })
}

pub fn verify_runner(runner: &Runner) -> Result<(), RunnerError> {
    if !runner.binary_path.is_file() {
        return Err(RunnerError::MissingBinary(runner.binary_path.clone()));
    }
    if !is_executable_file(&runner.binary_path) {
        return Err(RunnerError::NotExecutable(runner.binary_path.clone()));
    }
    Ok(())
}

async fn probe_wine(binary: &Path, timeout: Duration) -> Result<String, RunnerError> {
    let mut command = Command::new(binary);
    command.arg("--version").kill_on_drop(true);
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| RunnerError::ProbeTimeout(binary.to_path_buf()))??;
    if !output.status.success() {
        return Err(RunnerError::ProbeFailed {
            path: binary.to_path_buf(),
            code: output.status.code(),
            stderr: launcher_core::truncate_utf8(&String::from_utf8_lossy(&output.stderr), 4096),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn detect_from_candidates(candidates: &[PathBuf], timeout: Duration) -> Vec<Runner> {
    let mut runners = Vec::new();
    for candidate in candidates {
        match probe_wine(candidate, timeout).await {
            Ok(version) => runners.push(Runner {
                id: Uuid::new_v4(),
                kind: RunnerKind::Wine,
                name: "System Wine".into(),
                binary_path: candidate.clone(),
                root_dir: candidate
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("/")),
                arch: Arch::Win64,
                source: RunnerSource::System,
                verified: true,
                version_string: Some(version),
            }),
            Err(error) => {
                tracing::warn!(?candidate, %error, "skipping wine candidate");
            }
        }
    }
    runners
}

pub async fn detect_system_wine() -> Vec<Runner> {
    let path_env = std::env::var_os("PATH");
    let candidates = candidate_wine_paths(path_env.as_deref());
    detect_from_candidates(&candidates, Duration::from_secs(5)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, RunnerKind, RunnerSource};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn write_script(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn proton_directory_is_validated() {
        let dir = tempdir().unwrap();
        write_script(&dir.path().join("proton"), "#!/bin/sh\nexit 0\n");
        fs::write(dir.path().join("version"), "GE-Proton9-1\n").unwrap();

        let runner = validate_proton_dir(dir.path()).unwrap();
        assert_eq!(runner.kind, RunnerKind::Proton);
        assert_eq!(runner.arch, Arch::Win64);
        assert!(runner.verified);
        assert_eq!(runner.version_string.as_deref(), Some("GE-Proton9-1"));
        assert_eq!(
            runner.binary_path,
            dir.path().canonicalize().unwrap().join("proton")
        );
        assert!(matches!(runner.source, RunnerSource::Imported { .. }));
    }

    #[test]
    fn proton_without_version_file_uses_directory_name() {
        let parent = tempdir().unwrap();
        let dir = parent.path().join("GE-Proton9-1");
        fs::create_dir(&dir).unwrap();
        write_script(&dir.join("proton"), "#!/bin/sh\nexit 0\n");

        let runner = validate_proton_dir(&dir).unwrap();
        assert_eq!(runner.version_string.as_deref(), Some("GE-Proton9-1"));
    }

    #[test]
    fn proton_without_script_is_rejected() {
        let dir = tempdir().unwrap();
        assert!(matches!(
            validate_proton_dir(dir.path()),
            Err(RunnerError::InvalidProtonDir(_))
        ));
    }

    #[test]
    fn proton_script_must_be_executable() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("proton"), "#!/bin/sh\n").unwrap();
        let mut permissions = fs::metadata(dir.path().join("proton"))
            .unwrap()
            .permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(dir.path().join("proton"), permissions).unwrap();

        assert!(matches!(
            validate_proton_dir(dir.path()),
            Err(RunnerError::InvalidProtonDir(_))
        ));
    }

    #[test]
    fn verify_runner_checks_binary_presence_and_mode() {
        let dir = tempdir().unwrap();
        let binary = dir.path().join("wine");
        write_script(&binary, "#!/bin/sh\nexit 0\n");
        let runner = Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: "Wine".into(),
            binary_path: binary.clone(),
            root_dir: dir.path().to_path_buf(),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        };
        verify_runner(&runner).unwrap();

        fs::remove_file(&binary).unwrap();
        assert!(matches!(
            verify_runner(&runner),
            Err(RunnerError::MissingBinary(_))
        ));
    }

    #[test]
    fn candidate_paths_include_path_entry_and_injected_common_locations() {
        let path_dir = tempdir().unwrap();
        let common_dir = tempdir().unwrap();
        let wine = path_dir.path().join("wine");
        let common_wine = common_dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho wine-9.0\n");
        write_script(&common_wine, "#!/bin/sh\necho wine-9.0\n");

        let candidates = candidate_wine_paths_with(
            Some(path_dir.path().as_os_str()),
            std::slice::from_ref(&common_wine),
        );
        assert_eq!(candidates, vec![wine, common_wine]);
    }

    #[test]
    fn candidate_paths_deduplicate_symlinks() {
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho wine-9.0\n");
        let link = dir.path().join("wine-link");
        std::os::unix::fs::symlink(&wine, &link).unwrap();

        let candidates =
            candidate_wine_paths_with(Some(dir.path().as_os_str()), &[wine.clone(), link]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], wine);
    }

    #[tokio::test]
    async fn detect_reads_version_from_fake_wine() {
        let _exec_guard = crate::test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho wine-9.0-test\n");

        let runners =
            detect_from_candidates(std::slice::from_ref(&wine), Duration::from_secs(1)).await;
        assert_eq!(runners.len(), 1);
        assert_eq!(runners[0].version_string.as_deref(), Some("wine-9.0-test"));
        assert_eq!(runners[0].binary_path, wine);
        assert!(runners[0].verified);
    }

    #[tokio::test]
    async fn detect_skips_failing_probe() {
        let _exec_guard = crate::test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\necho boom >&2\nexit 3\n");

        let runners = detect_from_candidates(&[wine], Duration::from_secs(1)).await;
        assert!(runners.is_empty());
    }

    #[tokio::test]
    async fn detect_times_out_hung_probe() {
        let _exec_guard = crate::test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let wine = dir.path().join("wine");
        write_script(&wine, "#!/bin/sh\nsleep 5\n");

        let started = std::time::Instant::now();
        let runners = detect_from_candidates(&[wine], Duration::from_millis(50)).await;
        assert!(runners.is_empty());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
