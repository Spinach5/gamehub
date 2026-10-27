use launcher_core::{Arch, Game, Runner, RunnerKind};
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixPlan {
    pub path: PathBuf,
    pub arch: Arch,
    pub runner: Runner,
    pub steam_compat_client: PathBuf,
}

impl PrefixPlan {
    pub fn new(path: PathBuf, runner: Runner, arch: Arch, steam_compat_client: PathBuf) -> Self {
        Self {
            path,
            arch,
            runner,
            steam_compat_client,
        }
    }

    pub fn default_for(
        game_id: Uuid,
        default_root: &Path,
        runner: Runner,
        arch: Arch,
        steam_compat_client: PathBuf,
    ) -> Self {
        Self::new(
            default_prefix_path(game_id, default_root),
            runner,
            arch,
            steam_compat_client,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixStatus {
    Missing,
    NotInitialized,
    Initialized,
    ArchMismatch { existing: Option<Arch> },
}

pub fn default_prefix_path(game_id: Uuid, default_root: &Path) -> PathBuf {
    default_root.join(game_id.to_string())
}

pub fn resolve_prefix_path(game: &Game, default_root: &Path) -> PathBuf {
    game.prefix_path
        .clone()
        .unwrap_or_else(|| default_prefix_path(game.id, default_root))
}

pub fn plan_for_game(
    game: &Game,
    runner: Runner,
    default_root: &Path,
    steam_compat_client: PathBuf,
) -> PrefixPlan {
    let arch = match runner.kind {
        RunnerKind::Proton => Arch::Win64,
        RunnerKind::Wine => game.arch,
    };
    PrefixPlan::new(
        resolve_prefix_path(game, default_root),
        runner,
        arch,
        steam_compat_client,
    )
}

pub fn windows_root(prefix: &Path, kind: RunnerKind) -> PathBuf {
    match kind {
        RunnerKind::Wine => prefix.to_path_buf(),
        RunnerKind::Proton => prefix.join("pfx"),
    }
}

pub fn detect_arch(prefix: &Path, kind: RunnerKind) -> Option<Arch> {
    let system_reg = windows_root(prefix, kind).join("system.reg");
    let file = std::fs::File::open(system_reg).ok()?;
    let mut buffer = Vec::with_capacity(8192);
    file.take(8192).read_to_end(&mut buffer).ok()?;
    let text = String::from_utf8_lossy(&buffer);
    if text.contains("#arch=win64") {
        Some(Arch::Win64)
    } else if text.contains("#arch=win32") {
        Some(Arch::Win32)
    } else {
        None
    }
}

pub fn status(plan: &PrefixPlan) -> PrefixStatus {
    if !plan.path.exists() {
        return PrefixStatus::Missing;
    }
    let system_reg = windows_root(&plan.path, plan.runner.kind).join("system.reg");
    if !system_reg.is_file() {
        return PrefixStatus::NotInitialized;
    }
    match detect_arch(&plan.path, plan.runner.kind) {
        Some(existing) if existing != plan.arch => PrefixStatus::ArchMismatch {
            existing: Some(existing),
        },
        _ => PrefixStatus::Initialized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, Game, RunnerKind};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;
    use uuid::Uuid;

    fn plan(path: &Path, kind: RunnerKind, arch: Arch) -> PrefixPlan {
        PrefixPlan::new(
            path.to_path_buf(),
            test_runner(kind),
            arch,
            PathBuf::from("/data/steam-compat-client"),
        )
    }

    fn test_runner(kind: RunnerKind) -> launcher_core::Runner {
        launcher_core::Runner {
            id: Uuid::new_v4(),
            kind,
            name: "test".into(),
            binary_path: PathBuf::from("/usr/bin/wine"),
            root_dir: PathBuf::from("/usr"),
            arch: Arch::Win64,
            source: launcher_core::RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    #[test]
    fn default_prefix_path_uses_game_id() {
        let root = Path::new("/data/prefixes");
        let id = Uuid::nil();
        assert_eq!(
            default_prefix_path(id, root),
            PathBuf::from("/data/prefixes/00000000-0000-0000-0000-000000000000")
        );
    }

    #[test]
    fn explicit_game_prefix_wins() {
        let game = Game {
            id: Uuid::nil(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: Some(PathBuf::from("/shared/prefix")),
            cover_path: None,
            arch: Arch::Win64,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        };
        assert_eq!(
            resolve_prefix_path(&game, Path::new("/data/prefixes")),
            PathBuf::from("/shared/prefix")
        );
    }

    #[test]
    fn wine_plan_uses_game_arch() {
        let game = Game {
            id: Uuid::nil(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: None,
            cover_path: None,
            arch: Arch::Win32,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        };
        let plan = plan_for_game(
            &game,
            test_runner(RunnerKind::Wine),
            Path::new("/data/prefixes"),
            PathBuf::from("/data/steam-client"),
        );
        assert_eq!(plan.arch, Arch::Win32);
    }

    #[test]
    fn proton_plan_always_uses_win64() {
        let game = Game {
            id: Uuid::nil(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id: None,
            prefix_path: Some(PathBuf::from("/shared/prefix")),
            cover_path: None,
            arch: Arch::Win32,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
        };
        let plan = plan_for_game(
            &game,
            test_runner(RunnerKind::Proton),
            Path::new("/data/prefixes"),
            PathBuf::from("/data/steam-client"),
        );
        assert_eq!(plan.path, PathBuf::from("/shared/prefix"));
        assert_eq!(plan.arch, Arch::Win64);
    }

    #[test]
    fn windows_root_depends_on_runner_kind() {
        let prefix = Path::new("/data/prefixes/game");
        assert_eq!(
            windows_root(prefix, RunnerKind::Wine),
            PathBuf::from("/data/prefixes/game")
        );
        assert_eq!(
            windows_root(prefix, RunnerKind::Proton),
            PathBuf::from("/data/prefixes/game/pfx")
        );
    }

    #[test]
    fn detect_arch_reads_wine_system_reg() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("system.reg"), "#arch=win64\n").unwrap();
        assert_eq!(detect_arch(dir.path(), RunnerKind::Wine), Some(Arch::Win64));
    }

    #[test]
    fn detect_arch_reads_proton_pfx_system_reg() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("pfx")).unwrap();
        fs::write(dir.path().join("pfx/system.reg"), "#arch=win32\n").unwrap();
        assert_eq!(
            detect_arch(dir.path(), RunnerKind::Proton),
            Some(Arch::Win32)
        );
    }

    #[test]
    fn status_reports_missing_and_uninitialized() {
        let dir = tempdir().unwrap();
        let missing = plan(&dir.path().join("missing"), RunnerKind::Wine, Arch::Win64);
        assert_eq!(status(&missing), PrefixStatus::Missing);

        let uninitialized = plan(dir.path(), RunnerKind::Wine, Arch::Win64);
        assert_eq!(status(&uninitialized), PrefixStatus::NotInitialized);
    }

    #[test]
    fn status_detects_arch_mismatch() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("system.reg"), "#arch=win32\n").unwrap();
        assert_eq!(
            status(&plan(dir.path(), RunnerKind::Wine, Arch::Win64)),
            PrefixStatus::ArchMismatch {
                existing: Some(Arch::Win32)
            }
        );
    }

    #[test]
    fn status_treats_reg_without_arch_as_initialized() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("system.reg"), "WINE REGISTRY Version 2\n").unwrap();
        assert_eq!(
            status(&plan(dir.path(), RunnerKind::Wine, Arch::Win64)),
            PrefixStatus::Initialized
        );
    }
}
