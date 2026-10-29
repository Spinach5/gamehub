use std::path::Path;

/// Reports whether any process still runs with `WINEPREFIX` pointing at the
/// given prefix. Wine starts game processes from wineserver, so they can be
/// alive while nothing is left in the launcher's own process group.
///
/// Processes sharing the same prefix are indistinguishable here; a launcher
/// session for one game therefore stays open while another game from the same
/// prefix is still running.
pub fn prefix_has_processes(wine_prefix: &Path) -> bool {
    let expected = format!("WINEPREFIX={}", wine_prefix.to_string_lossy()).into_bytes();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };

    entries.flatten().any(|entry| {
        let process_dir = entry.path();
        let is_pid = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()));
        is_pid
            && !is_wineserver(&process_dir)
            && std::fs::read(process_dir.join("environ")).is_ok_and(|environ| {
                environ
                    .split(|byte| *byte == 0)
                    .any(|variable| variable == expected.as_slice())
            })
    })
}

/// `wineserver` inherits the prefix environment and keeps idling for a few
/// seconds after its last client exits, so it must not count as a live game.
fn is_wineserver(process_dir: &Path) -> bool {
    std::fs::read_to_string(process_dir.join("comm"))
        .is_ok_and(|comm| comm.trim_end() == "wineserver")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::tempdir;

    async fn wait_for_prefix_busy(wine_prefix: &Path, expected: bool) -> bool {
        for _ in 0..200 {
            if prefix_has_processes(wine_prefix) == expected {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    #[test]
    fn unused_prefix_has_no_processes() {
        assert!(!prefix_has_processes(Path::new(
            "/tmp/gamehub-nonexistent-prefix"
        )));
    }

    #[tokio::test]
    async fn finds_processes_that_carry_the_prefix() {
        let dir = tempdir().unwrap();
        let wine_prefix: PathBuf = dir.path().join("pfx");
        let mut child = tokio::process::Command::new("sleep")
            .arg("30")
            .env("WINEPREFIX", &wine_prefix)
            .spawn()
            .unwrap();

        assert!(wait_for_prefix_busy(&wine_prefix, true).await);

        let _ = child.kill().await;
        let _ = child.wait().await;
        assert!(wait_for_prefix_busy(&wine_prefix, false).await);
    }

    #[tokio::test]
    async fn an_idling_wineserver_does_not_count_as_a_game() {
        let _guard = crate::test_support::EXEC_LOCK.lock().await;
        let dir = tempdir().unwrap();
        let wine_prefix: PathBuf = dir.path().join("pfx");
        let wineserver = dir.path().join("wineserver");
        std::fs::copy("/bin/sleep", &wineserver).unwrap();
        let mut child = tokio::process::Command::new(&wineserver)
            .arg("30")
            .env("WINEPREFIX", &wine_prefix)
            .spawn()
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!prefix_has_processes(&wine_prefix));

        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}
