//! Recursive, cancellable scan for `.dll` files below a directory.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

pub const DLL_SCAN_MAX_DEPTH: usize = 16;
pub const DLL_SCAN_MAX_RESULTS: usize = 5_000;
pub const DLL_SCAN_MAX_VISITED_FILES: usize = 200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllCandidate {
    /// File name without the `.dll` suffix, original case preserved.
    pub name: String,
    /// Lowercased dedupe key produced by [`dll_name_key`].
    pub key: String,
    /// Path relative to the scan root, used to tell same-named files apart.
    pub relative_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllScanOutcome {
    pub entries: Vec<DllCandidate>,
    pub skipped_non_utf8: usize,
    pub unreadable_dirs: usize,
    pub truncated: bool,
    pub depth_limited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DllScanError {
    #[error("directory not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("path is not a directory: {}", .0.display())]
    NotDirectory(PathBuf),
    #[error("permission denied: {}", .0.display())]
    PermissionDenied(PathBuf),
    #[error("failed to read {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("scan cancelled")]
    Cancelled,
    #[error("scan task failed: {0}")]
    Join(String),
}

/// `trim` → strip a case-insensitive `.dll` suffix → `trim_end` → lowercase.
pub fn dll_name_key(name: &str) -> String {
    let trimmed = name.trim();
    let stem = if trimmed.len() >= 4
        && trimmed.as_bytes()[trimmed.len() - 4..].eq_ignore_ascii_case(b".dll")
    {
        trimmed[..trimmed.len() - 4].trim_end()
    } else {
        trimmed
    };
    stem.to_ascii_lowercase()
}

#[derive(Debug, Clone, Copy)]
struct ScanLimits {
    max_depth: usize,
    max_results: usize,
    max_visited_files: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_depth: DLL_SCAN_MAX_DEPTH,
            max_results: DLL_SCAN_MAX_RESULTS,
            max_visited_files: DLL_SCAN_MAX_VISITED_FILES,
        }
    }
}

/// Blocking scan; callers must run it inside `spawn_blocking`.
pub fn scan_dlls(root: &Path, cancel: &AtomicBool) -> Result<DllScanOutcome, DllScanError> {
    scan_with_limits(root, cancel, ScanLimits::default())
}

fn scan_with_limits(
    root: &Path,
    cancel: &AtomicBool,
    limits: ScanLimits,
) -> Result<DllScanOutcome, DllScanError> {
    let metadata = fs::metadata(root).map_err(|error| map_io_error(root, error))?;
    if !metadata.is_dir() {
        return Err(DllScanError::NotDirectory(root.to_path_buf()));
    }

    let mut outcome = DllScanOutcome {
        entries: Vec::new(),
        skipped_non_utf8: 0,
        unreadable_dirs: 0,
        truncated: false,
        depth_limited: false,
    };
    let mut seen = HashSet::new();
    let mut visited_files = 0usize;
    let mut stack = vec![(root.to_path_buf(), 0usize)];

    'walk: while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err(DllScanError::Cancelled);
        }

        let read = match fs::read_dir(&dir) {
            Ok(read) => read,
            Err(error) if dir.as_path() == root => {
                return Err(map_io_error(&dir, error));
            }
            Err(_) => {
                // `read_dir` reports unreadable directories the same way.
                outcome.unreadable_dirs += 1;
                continue;
            }
        };

        let mut children = Vec::new();
        for entry in read {
            match entry {
                Ok(entry) => children.push(entry),
                Err(_) => outcome.unreadable_dirs += 1,
            }
        }
        children.sort_by_key(|entry| entry.file_name());

        for entry in children {
            if cancel.load(Ordering::Relaxed) {
                return Err(DllScanError::Cancelled);
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            // Never follow symlinks: this also makes directory loops harmless.
            if file_type.is_symlink() {
                continue;
            }

            let path = entry.path();
            if file_type.is_dir() {
                if depth >= limits.max_depth {
                    outcome.depth_limited = true;
                } else {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            visited_files += 1;
            if visited_files > limits.max_visited_files {
                outcome.truncated = true;
                break 'walk;
            }
            if !is_dll_file(&path) {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
                outcome.skipped_non_utf8 += 1;
                continue;
            };
            let name = file_name[..file_name.len() - 4].to_string();
            if name.is_empty() {
                continue;
            }
            let Ok(relative_path) = path.strip_prefix(root) else {
                continue;
            };
            if relative_path.to_str().is_none() {
                outcome.skipped_non_utf8 += 1;
                continue;
            }

            let key = dll_name_key(&name);
            if !seen.insert(key.clone()) {
                continue;
            }
            if outcome.entries.len() >= limits.max_results {
                outcome.truncated = true;
                break 'walk;
            }
            outcome.entries.push(DllCandidate {
                name,
                key,
                relative_path: relative_path.to_path_buf(),
            });
        }
    }

    outcome.entries.sort_by(|a, b| {
        a.key
            .cmp(&b.key)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    Ok(outcome)
}

fn is_dll_file(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
}

fn map_io_error(path: &Path, error: std::io::Error) -> DllScanError {
    match error.kind() {
        std::io::ErrorKind::NotFound => DllScanError::NotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => DllScanError::PermissionDenied(path.to_path_buf()),
        _ => DllScanError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"stub").unwrap();
    }

    fn limits(max_depth: usize, max_results: usize, max_visited_files: usize) -> ScanLimits {
        ScanLimits {
            max_depth,
            max_results,
            max_visited_files,
        }
    }

    #[test]
    fn dll_name_key_strips_suffix_and_normalizes_case() {
        assert_eq!(dll_name_key("d3d11.dll"), "d3d11");
        assert_eq!(dll_name_key("D3D11.DLL"), "d3d11");
        assert_eq!(dll_name_key("  WinMM.dll  "), "winmm");
        assert_eq!(dll_name_key("  D3D11  "), "d3d11");
        assert_eq!(dll_name_key(""), "");
    }

    #[test]
    fn collects_recursively_and_dedupes_by_basename() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("a.dll"));
        touch(&root.path().join("notes.txt"));
        touch(&root.path().join("sub/B.DLL"));
        touch(&root.path().join("sub/deep/a.dll"));

        let outcome = scan_dlls(root.path(), &AtomicBool::new(false)).unwrap();
        let keys: Vec<_> = outcome
            .entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(outcome.entries[0].relative_path, PathBuf::from("a.dll"));
        assert_eq!(outcome.entries[1].name, "B");
        assert!(!outcome.truncated);
        assert!(!outcome.depth_limited);
    }

    #[test]
    fn depth_limit_reports_and_skips_deeper_directories() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("top.dll"));
        touch(&root.path().join("sub/inner.dll"));
        touch(&root.path().join("sub/deep/too_deep.dll"));

        let outcome =
            scan_with_limits(root.path(), &AtomicBool::new(false), limits(1, 100, 100)).unwrap();
        let keys: Vec<_> = outcome
            .entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        assert_eq!(keys, vec!["inner", "top"]);
        assert!(outcome.depth_limited);
    }

    #[test]
    fn result_cap_sets_truncated() {
        let root = TempDir::new().unwrap();
        for name in ["a.dll", "b.dll", "c.dll"] {
            touch(&root.path().join(name));
        }
        let outcome =
            scan_with_limits(root.path(), &AtomicBool::new(false), limits(16, 2, 100)).unwrap();
        assert_eq!(outcome.entries.len(), 2);
        assert!(outcome.truncated);
    }

    #[test]
    fn visited_file_cap_sets_truncated() {
        let root = TempDir::new().unwrap();
        for name in ["a.txt", "b.dll", "c.dll"] {
            touch(&root.path().join(name));
        }
        let outcome =
            scan_with_limits(root.path(), &AtomicBool::new(false), limits(16, 100, 2)).unwrap();
        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(outcome.entries[0].key, "b");
        assert!(outcome.truncated);
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_file_names_are_skipped_and_counted() {
        use std::os::unix::ffi::OsStringExt;

        let root = TempDir::new().unwrap();
        let name = std::ffi::OsString::from_vec(b"bad\xFF.dll".to_vec());
        fs::write(root.path().join(name), b"stub").unwrap();

        let outcome = scan_dlls(root.path(), &AtomicBool::new(false)).unwrap();
        assert!(outcome.entries.is_empty());
        assert_eq!(outcome.skipped_non_utf8, 1);
    }

    #[test]
    fn missing_and_non_directory_roots_are_reported() {
        let root = TempDir::new().unwrap();
        let missing = root.path().join("missing");
        assert_eq!(
            scan_dlls(&missing, &AtomicBool::new(false)).unwrap_err(),
            DllScanError::NotFound(missing)
        );

        let file = root.path().join("game.exe");
        fs::write(&file, b"stub").unwrap();
        assert_eq!(
            scan_dlls(&file, &AtomicBool::new(false)).unwrap_err(),
            DllScanError::NotDirectory(file)
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlink_loops_do_not_recurse_forever() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("a.dll"));
        std::os::unix::fs::symlink(root.path(), root.path().join("loop")).unwrap();

        let outcome = scan_dlls(root.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(outcome.entries.len(), 1);
    }

    #[test]
    fn cancellation_is_reported_before_walking() {
        let root = TempDir::new().unwrap();
        touch(&root.path().join("a.dll"));
        let cancel = AtomicBool::new(true);
        assert_eq!(
            scan_dlls(root.path(), &cancel).unwrap_err(),
            DllScanError::Cancelled
        );
    }
}
