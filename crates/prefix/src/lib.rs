mod dxvk;
mod error;
mod initialize;
mod nvapi;
mod status;

#[cfg(test)]
mod test_support;

pub use dxvk::{
    DxvkFile, DxvkSource, DxvkTarget, dxvk_registry_entries, install_dxvk, validate_dxvk_source,
};
pub use error::PrefixError;
pub use nvapi::{DxvkNvapiSource, install_dxvk_nvapi};
pub use status::{
    PrefixPlan, PrefixStatus, default_prefix_path, detect_arch, plan_for_game, resolve_prefix_path,
    status, windows_root,
};

use std::path::Path;

pub async fn delete_prefix(path: &Path) -> Result<(), PrefixError> {
    if !path.exists() {
        return Ok(());
    }
    let initialized = path.join("system.reg").is_file() || path.join("pfx/system.reg").is_file();
    if !initialized {
        return Err(PrefixError::RefusingToDeleteNonPrefix(path.to_path_buf()));
    }
    tokio::fs::remove_dir_all(path)
        .await
        .map_err(PrefixError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn delete_prefix_removes_tree_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("pfx")).unwrap();
        fs::write(prefix.join("pfx/system.reg"), "x").unwrap();

        delete_prefix(&prefix).await.unwrap();
        assert!(!prefix.exists());
        delete_prefix(&prefix).await.unwrap();
    }

    #[tokio::test]
    async fn delete_prefix_refuses_arbitrary_directory() {
        let dir = tempdir().unwrap();
        let victim = dir.path().join("not-a-prefix");
        fs::create_dir(&victim).unwrap();
        fs::write(victim.join("precious.txt"), "keep me").unwrap();

        assert!(matches!(
            delete_prefix(&victim).await,
            Err(PrefixError::RefusingToDeleteNonPrefix(_))
        ));
        assert!(victim.exists());
    }
}
