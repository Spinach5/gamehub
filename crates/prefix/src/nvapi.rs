use crate::dxvk::DxvkTarget;
use crate::{PrefixError, PrefixPlan, PrefixStatus, status, windows_root};
use launcher_core::{Arch, RunnerKind, truncate_utf8};
use runner_wine::WineContext;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub(crate) const MARKER: &str = ".gamehub-dxvk-nvapi";
const REG_TIMEOUT: Duration = Duration::from_secs(30);
const REG_VALUE: &str = "native,builtin";
const X64_DLLS: [(&str, &str); 2] = [("nvapi64", "nvapi64.dll"), ("nvofapi64", "nvofapi64.dll")];
const X32_DLLS: [(&str, &str); 2] = [("nvapi", "nvapi.dll"), ("nvofapi", "nvofapi.dll")];

#[derive(Debug, Clone)]
pub struct DxvkNvapiSource {
    pub root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DxvkNvapiFile {
    source: PathBuf,
    target: DxvkTarget,
    dll_stem: &'static str,
}

fn collect_dir(
    dir: &Path,
    target: DxvkTarget,
    required: Option<&str>,
    candidates: &[(&'static str, &'static str)],
    files: &mut Vec<DxvkNvapiFile>,
) -> Result<(), PrefixError> {
    if let Some(name) = required {
        let path = dir.join(name);
        if !path.is_file() {
            return Err(PrefixError::DxvkNvapiSourceInvalid { path });
        }
    }

    for (stem, name) in candidates {
        let path = dir.join(name);
        if path.is_file() {
            files.push(DxvkNvapiFile {
                source: path,
                target,
                dll_stem: stem,
            });
        }
    }
    Ok(())
}

fn validate_dxvk_nvapi_source(
    source: &DxvkNvapiSource,
    arch: Arch,
) -> Result<Vec<DxvkNvapiFile>, PrefixError> {
    let mut files = Vec::new();
    match arch {
        Arch::Win64 => {
            collect_dir(
                &source.root.join("x64"),
                DxvkTarget::System32,
                Some("nvapi64.dll"),
                &X64_DLLS,
                &mut files,
            )?;
            collect_dir(
                &source.root.join("x32"),
                DxvkTarget::SysWow64,
                None,
                &X32_DLLS,
                &mut files,
            )?;
        }
        Arch::Win32 => {
            collect_dir(
                &source.root.join("x32"),
                DxvkTarget::System32,
                Some("nvapi.dll"),
                &X32_DLLS,
                &mut files,
            )?;
        }
    }
    Ok(files)
}

fn nvapi_registry_entries(files: &[DxvkNvapiFile]) -> Vec<(&'static str, &'static str)> {
    let mut stems: Vec<&'static str> = Vec::new();
    for file in files {
        if !stems.contains(&file.dll_stem) {
            stems.push(file.dll_stem);
        }
    }
    stems.into_iter().map(|stem| (stem, REG_VALUE)).collect()
}

pub async fn install_dxvk_nvapi(
    plan: &PrefixPlan,
    source: &DxvkNvapiSource,
) -> Result<(), PrefixError> {
    if plan.runner.kind == RunnerKind::Proton {
        return Ok(());
    }

    if status(plan) != PrefixStatus::Initialized {
        return Err(PrefixError::PrefixNotInitialized(plan.path.clone()));
    }

    let files = validate_dxvk_nvapi_source(source, plan.arch)?;
    let windows = windows_root(&plan.path, plan.runner.kind).join("drive_c/windows");
    let marker = plan.path.join(MARKER);
    let marker_present = marker.is_file();
    let files_for_copy = files.clone();

    let copied = tokio::task::spawn_blocking(move || -> Result<bool, std::io::Error> {
        let mut copied = false;
        for file in &files_for_copy {
            let file_name = file.source.file_name().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid dxvk-nvapi source path: {:?}", file.source),
                )
            })?;
            let target_dir = windows.join(file.target.dir_name());
            let target = target_dir.join(file_name);
            let source_bytes = std::fs::read(&file.source)?;
            let unchanged = std::fs::read(&target)
                .map(|existing| existing == source_bytes)
                .unwrap_or(false);
            if !unchanged {
                std::fs::create_dir_all(&target_dir)?;
                std::fs::copy(&file.source, &target)?;
                copied = true;
            }
        }
        Ok(copied)
    })
    .await
    .map_err(|error| PrefixError::Join(error.to_string()))??;

    if !copied && marker_present {
        return Ok(());
    }

    let context = WineContext {
        runner: plan.runner.clone(),
        prefix: plan.path.clone(),
        arch: plan.arch,
        steam_compat_client: plan.steam_compat_client.clone(),
    };

    for (stem, value) in nvapi_registry_entries(&files) {
        let args: Vec<String> = [
            "add",
            "HKCU\\Software\\Wine\\DllOverrides",
            "/v",
            stem,
            "/d",
            value,
            "/f",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let (program, command_args) = context.build_command(Path::new("reg"), &args);

        let mut command = Command::new(&program);
        command
            .args(&command_args)
            .envs(context.base_env())
            .kill_on_drop(true)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        let output = tokio::time::timeout(REG_TIMEOUT, command.output())
            .await
            .map_err(|_| PrefixError::RegAddTimeout {
                dll: stem.to_string(),
            })??;

        if !output.status.success() {
            return Err(PrefixError::RegAddFailed {
                dll: stem.to_string(),
                code: output.status.code(),
                stderr: truncate_utf8(&String::from_utf8_lossy(&output.stderr), 4096),
            });
        }
    }

    tokio::fs::write(&marker, b"").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{PrefixError, PrefixPlan, test_support};
    use launcher_core::{Arch, RunnerKind};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_dll(root: &Path, dir: &str, name: &str, body: &[u8]) {
        let dir = root.join(dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), body).unwrap();
    }

    fn initialized_prefix(dir: &Path, arch: &str) -> std::path::PathBuf {
        let prefix = dir.join("prefix");
        fs::create_dir_all(&prefix).unwrap();
        fs::write(prefix.join("system.reg"), format!("#arch={arch}\n")).unwrap();
        prefix
    }

    fn source(root: &Path) -> super::DxvkNvapiSource {
        super::DxvkNvapiSource {
            root: root.to_path_buf(),
        }
    }

    #[test]
    fn win64_maps_x64_to_system32_and_x32_to_syswow64() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        write_dll(root, "x64", "nvapi64.dll", b"v64");
        write_dll(root, "x64", "nvofapi64.dll", b"o64");
        write_dll(root, "x32", "nvapi.dll", b"v32");
        write_dll(root, "x32", "nvofapi.dll", b"o32");

        let files = super::validate_dxvk_nvapi_source(&source(root), Arch::Win64).unwrap();
        assert_eq!(files.len(), 4);
        assert_eq!(
            files
                .iter()
                .filter(|file| file.target.dir_name() == "system32")
                .count(),
            2
        );
        assert_eq!(
            files
                .iter()
                .filter(|file| file.target.dir_name() == "syswow64")
                .count(),
            2
        );
    }

    #[test]
    fn win64_without_x32_is_valid() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x64", "nvapi64.dll", b"v64");

        let files = super::validate_dxvk_nvapi_source(&source(dir.path()), Arch::Win64).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].target.dir_name(), "system32");
    }

    #[test]
    fn win32_requires_x32_nvapi() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x32", "nvofapi.dll", b"o32");

        assert!(matches!(
            super::validate_dxvk_nvapi_source(&source(dir.path()), Arch::Win32),
            Err(PrefixError::DxvkNvapiSourceInvalid { path })
                if path.ends_with("x32/nvapi.dll")
        ));
    }

    #[test]
    fn win32_maps_x32_to_system32() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x32", "nvapi.dll", b"v32");
        write_dll(dir.path(), "x32", "nvofapi.dll", b"o32");

        let files = super::validate_dxvk_nvapi_source(&source(dir.path()), Arch::Win32).unwrap();
        assert_eq!(files.len(), 2);
        assert!(
            files
                .iter()
                .all(|file| file.target.dir_name() == "system32")
        );
    }

    #[test]
    fn unrelated_files_are_ignored() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x64", "nvapi64.dll", b"v64");
        write_dll(dir.path(), "x64", "dxvk.dll", b"not nvapi");
        write_dll(dir.path(), "x64", "nvapi64.pdb", b"debug");
        write_dll(dir.path(), "x64", "layer.dll", b"layer");

        let files = super::validate_dxvk_nvapi_source(&source(dir.path()), Arch::Win64).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].source.ends_with("x64/nvapi64.dll"));
    }

    #[test]
    fn registry_entries_are_unique_by_stem() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        write_dll(root, "x64", "nvapi64.dll", b"v64");
        write_dll(root, "x32", "nvapi.dll", b"v32");
        write_dll(root, "x32", "nvofapi.dll", b"o32");

        let files = super::validate_dxvk_nvapi_source(&source(root), Arch::Win64).unwrap();
        let entries = super::nvapi_registry_entries(&files);
        let stems: Vec<&str> = entries.iter().map(|(stem, _)| *stem).collect();
        assert_eq!(stems, vec!["nvapi64", "nvapi", "nvofapi"]);
        assert!(entries.iter().all(|(_, value)| *value == "native,builtin"));
    }

    #[tokio::test]
    async fn install_copies_dlls_and_writes_marker() {
        let _guard = test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let prefix = initialized_prefix(dir.path(), "win64");
        let reg_log = dir.path().join("reg.log");
        let wine = dir.path().join("wine");
        test_support::write_script(
            &wine,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
                reg_log.display()
            ),
        );

        let root = dir.path().join("nvapi");
        write_dll(&root, "x64", "nvapi64.dll", b"v64");
        write_dll(&root, "x64", "nvofapi64.dll", b"o64");
        write_dll(&root, "x32", "nvapi.dll", b"v32");

        let plan = PrefixPlan::new(
            prefix.clone(),
            test_support::fake_runner(RunnerKind::Wine, wine),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();

        assert_eq!(
            fs::read(prefix.join("drive_c/windows/system32/nvapi64.dll")).unwrap(),
            b"v64"
        );
        assert_eq!(
            fs::read(prefix.join("drive_c/windows/system32/nvofapi64.dll")).unwrap(),
            b"o64"
        );
        assert_eq!(
            fs::read(prefix.join("drive_c/windows/syswow64/nvapi.dll")).unwrap(),
            b"v32"
        );
        assert!(prefix.join(super::MARKER).is_file());

        let logged = fs::read_to_string(&reg_log).unwrap();
        assert!(logged.contains("HKCU\\Software\\Wine\\DllOverrides"));
        assert!(logged.contains("nvapi64"));
        assert!(logged.contains("nvofapi64"));
        assert!(logged.contains("nvapi"));
        assert_eq!(logged.matches("native,builtin").count(), 3);
    }

    #[tokio::test]
    async fn second_install_with_unchanged_files_skips_registry() {
        let _guard = test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let prefix = initialized_prefix(dir.path(), "win64");
        let reg_log = dir.path().join("reg.log");
        let wine = dir.path().join("wine");
        test_support::write_script(
            &wine,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
                reg_log.display()
            ),
        );

        let root = dir.path().join("nvapi");
        write_dll(&root, "x64", "nvapi64.dll", b"v64");

        let plan = PrefixPlan::new(
            prefix.clone(),
            test_support::fake_runner(RunnerKind::Wine, wine),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();
        let first = fs::read_to_string(&reg_log).unwrap();
        assert_eq!(first.matches("native,builtin").count(), 1);

        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();
        let second = fs::read_to_string(&reg_log).unwrap();
        assert_eq!(second, first);
    }

    #[tokio::test]
    async fn changed_dll_is_reinstalled_and_registered() {
        let _guard = test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let prefix = initialized_prefix(dir.path(), "win64");
        let reg_log = dir.path().join("reg.log");
        let wine = dir.path().join("wine");
        test_support::write_script(
            &wine,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
                reg_log.display()
            ),
        );

        let root = dir.path().join("nvapi");
        write_dll(&root, "x64", "nvapi64.dll", b"v1");
        let plan = PrefixPlan::new(
            prefix.clone(),
            test_support::fake_runner(RunnerKind::Wine, wine),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();
        write_dll(&root, "x64", "nvapi64.dll", b"v2");
        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();

        assert_eq!(
            fs::read(prefix.join("drive_c/windows/system32/nvapi64.dll")).unwrap(),
            b"v2"
        );
        let logged = fs::read_to_string(&reg_log).unwrap();
        assert_eq!(logged.matches("native,builtin").count(), 2);
    }

    #[tokio::test]
    async fn missing_marker_forces_registry_even_when_files_match() {
        let _guard = test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let prefix = initialized_prefix(dir.path(), "win64");
        let system32 = prefix.join("drive_c/windows/system32");
        fs::create_dir_all(&system32).unwrap();
        fs::write(system32.join("nvapi64.dll"), b"v64").unwrap();

        let reg_log = dir.path().join("reg.log");
        let wine = dir.path().join("wine");
        test_support::write_script(
            &wine,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
                reg_log.display()
            ),
        );

        let root = dir.path().join("nvapi");
        write_dll(&root, "x64", "nvapi64.dll", b"v64");
        let plan = PrefixPlan::new(
            prefix.clone(),
            test_support::fake_runner(RunnerKind::Wine, wine),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();

        assert!(prefix.join(super::MARKER).is_file());
        assert_eq!(
            fs::read_to_string(&reg_log)
                .unwrap()
                .matches("native,builtin")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn proton_prefixes_are_skipped() {
        let _guard = test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let prefix = dir.path().join("compat-data");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "#arch=win64\n").unwrap();

        let root = dir.path().join("nvapi");
        write_dll(&root, "x64", "nvapi64.dll", b"v64");
        let plan = PrefixPlan::new(
            prefix.clone(),
            test_support::fake_runner(RunnerKind::Proton, dir.path().join("proton")),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        super::install_dxvk_nvapi(&plan, &source(&root))
            .await
            .unwrap();

        assert!(
            !prefix
                .join("pfx/drive_c/windows/system32/nvapi64.dll")
                .exists()
        );
        assert!(!prefix.join(super::MARKER).exists());
    }

    #[tokio::test]
    async fn install_requires_initialized_prefix() {
        let dir = tempdir().unwrap();
        let root = dir.path().join("nvapi");
        write_dll(&root, "x64", "nvapi64.dll", b"v64");

        let plan = PrefixPlan::new(
            dir.path().join("missing"),
            test_support::fake_runner(RunnerKind::Wine, std::path::PathBuf::from("/bin/true")),
            Arch::Win64,
            dir.path().join("steam-client"),
        );

        assert!(matches!(
            super::install_dxvk_nvapi(&plan, &source(&root)).await,
            Err(PrefixError::PrefixNotInitialized(_))
        ));
    }
}
