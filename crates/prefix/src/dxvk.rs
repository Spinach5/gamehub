use crate::{PrefixError, PrefixPlan, PrefixStatus, status, windows_root};
use launcher_core::{Arch, truncate_utf8};
use runner_wine::WineContext;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

const DLLS: [(&str, &str); 4] = [
    ("d3d11", "d3d11.dll"),
    ("dxgi", "dxgi.dll"),
    ("d3d10core", "d3d10core.dll"),
    ("d3d9", "d3d9.dll"),
];
const REQUIRED: [(&str, &str); 2] = [("d3d11", "d3d11.dll"), ("dxgi", "dxgi.dll")];
const REG_TIMEOUT: Duration = Duration::from_secs(30);
const DXVK_REG_VALUE: &str = "native,builtin";

#[derive(Debug, Clone)]
pub struct DxvkSource {
    pub root: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxvkTarget {
    System32,
    SysWow64,
}

impl DxvkTarget {
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::System32 => "system32",
            Self::SysWow64 => "syswow64",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxvkFile {
    pub source: PathBuf,
    pub target: DxvkTarget,
    pub dll_stem: &'static str,
}

fn collect_dir(
    dir: &Path,
    target: DxvkTarget,
    required: bool,
    files: &mut Vec<DxvkFile>,
) -> Result<(), PrefixError> {
    if !dir.is_dir() {
        if required {
            return Err(PrefixError::DxvkSourceInvalid {
                path: dir.join(REQUIRED[0].1),
            });
        }
        return Ok(());
    }

    if required {
        for (_, file_name) in REQUIRED {
            let path = dir.join(file_name);
            if !path.is_file() {
                return Err(PrefixError::DxvkSourceInvalid { path });
            }
        }
    }

    for (stem, file_name) in DLLS {
        let path = dir.join(file_name);
        if path.is_file() {
            files.push(DxvkFile {
                source: path,
                target,
                dll_stem: stem,
            });
        }
    }
    Ok(())
}

pub fn validate_dxvk_source(source: &DxvkSource, arch: Arch) -> Result<Vec<DxvkFile>, PrefixError> {
    let mut files = Vec::new();
    match arch {
        Arch::Win64 => {
            collect_dir(
                &source.root.join("x64"),
                DxvkTarget::System32,
                true,
                &mut files,
            )?;
            collect_dir(
                &source.root.join("x32"),
                DxvkTarget::SysWow64,
                false,
                &mut files,
            )?;
        }
        Arch::Win32 => {
            collect_dir(
                &source.root.join("x32"),
                DxvkTarget::System32,
                true,
                &mut files,
            )?;
        }
    }
    Ok(files)
}

pub fn dxvk_registry_entries(files: &[DxvkFile]) -> Vec<(&'static str, &'static str)> {
    let mut stems: Vec<&'static str> = Vec::new();
    for file in files {
        if !stems.contains(&file.dll_stem) {
            stems.push(file.dll_stem);
        }
    }
    stems
        .into_iter()
        .map(|stem| (stem, DXVK_REG_VALUE))
        .collect()
}

pub async fn install_dxvk(plan: &PrefixPlan, source: &DxvkSource) -> Result<(), PrefixError> {
    if status(plan) != PrefixStatus::Initialized {
        return Err(PrefixError::PrefixNotInitialized(plan.path.clone()));
    }

    let files = validate_dxvk_source(source, plan.arch)?;
    let windows = windows_root(&plan.path, plan.runner.kind).join("drive_c/windows");
    let files_for_copy = files.clone();

    tokio::task::spawn_blocking(move || -> Result<(), std::io::Error> {
        for file in &files_for_copy {
            let target_dir = windows.join(file.target.dir_name());
            std::fs::create_dir_all(&target_dir)?;
            let file_name = file.source.file_name().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid dxvk source path: {:?}", file.source),
                )
            })?;
            std::fs::copy(&file.source, target_dir.join(file_name))?;
        }
        Ok(())
    })
    .await
    .map_err(|error| PrefixError::Join(error.to_string()))??;

    let context = WineContext {
        runner: plan.runner.clone(),
        prefix: plan.path.clone(),
        arch: plan.arch,
        steam_compat_client: plan.steam_compat_client.clone(),
    };

    for (stem, value) in dxvk_registry_entries(&files) {
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::Arch;
    use std::fs;
    use tempfile::tempdir;

    fn write_dll(root: &Path, dir: &str, name: &str) {
        let dir = root.join(dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), b"dll").unwrap();
    }

    #[test]
    fn win64_maps_x64_to_system32_and_x32_to_syswow64() {
        let dir = tempdir().unwrap();
        for name in ["d3d11.dll", "dxgi.dll", "d3d10core.dll", "d3d9.dll"] {
            write_dll(dir.path(), "x64", name);
            write_dll(dir.path(), "x32", name);
        }

        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        )
        .unwrap();

        assert_eq!(files.len(), 8);
        assert_eq!(
            files
                .iter()
                .filter(|file| file.target == DxvkTarget::System32)
                .count(),
            4
        );
        assert_eq!(
            files
                .iter()
                .filter(|file| file.target == DxvkTarget::SysWow64)
                .count(),
            4
        );
    }

    #[test]
    fn win64_without_x32_is_valid() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x64", "d3d11.dll");
        write_dll(dir.path(), "x64", "dxgi.dll");

        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| file.target == DxvkTarget::System32));
    }

    #[test]
    fn win32_maps_x32_to_system32() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x32", "d3d11.dll");
        write_dll(dir.path(), "x32", "dxgi.dll");

        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win32,
        )
        .unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|file| file.target == DxvkTarget::System32));
    }

    #[test]
    fn missing_required_dll_is_rejected() {
        let dir = tempdir().unwrap();
        write_dll(dir.path(), "x64", "dxgi.dll");

        match validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        ) {
            Err(PrefixError::DxvkSourceInvalid { path }) => {
                assert!(path.ends_with("x64/d3d11.dll"));
            }
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn registry_entries_are_unique_and_deterministic() {
        let dir = tempdir().unwrap();
        for name in ["d3d11.dll", "dxgi.dll", "d3d9.dll"] {
            write_dll(dir.path(), "x64", name);
            write_dll(dir.path(), "x32", name);
        }
        let files = validate_dxvk_source(
            &DxvkSource {
                root: dir.path().to_path_buf(),
            },
            Arch::Win64,
        )
        .unwrap();
        assert_eq!(
            dxvk_registry_entries(&files),
            vec![
                ("d3d11", "native,builtin"),
                ("dxgi", "native,builtin"),
                ("d3d9", "native,builtin"),
            ]
        );
    }
}
