use launcher_core::{Arch, Runner, RunnerKind, RunnerSource};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Executing a freshly written script while a concurrent fork still holds its
/// write fd yields ETXTBSY; tests that spawn scripts serialize here.
pub(crate) static EXEC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) fn write_script(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

pub(crate) fn fake_runner(kind: RunnerKind, binary_path: PathBuf) -> Runner {
    let root_dir = binary_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));
    Runner {
        id: Uuid::new_v4(),
        kind,
        name: "fake".into(),
        binary_path,
        root_dir,
        arch: Arch::Win64,
        source: RunnerSource::System,
        verified: true,
        version_string: None,
    }
}
