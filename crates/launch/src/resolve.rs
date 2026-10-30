use launcher_core::{Game, Runner};
use uuid::Uuid;

use crate::LaunchError;

pub fn resolve_runner(
    game: &Game,
    runners: &[Runner],
    default_runner_id: Option<Uuid>,
) -> Result<Runner, LaunchError> {
    if let Some(id) = game.runner_id {
        return runners
            .iter()
            .find(|runner| runner.id == id)
            .cloned()
            .ok_or(LaunchError::RunnerNotFound(id));
    }

    let id = default_runner_id.ok_or(LaunchError::NoRunnerSelected)?;
    runners
        .iter()
        .find(|runner| runner.id == id)
        .cloned()
        .ok_or(LaunchError::DefaultRunnerUnavailable(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::{Arch, Platform, RunnerKind, RunnerSource};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn runner(name: &str) -> Runner {
        Runner {
            id: Uuid::new_v4(),
            kind: RunnerKind::Wine,
            name: name.into(),
            binary_path: PathBuf::from("/usr/bin/wine"),
            root_dir: PathBuf::from("/usr/bin"),
            arch: Arch::Win64,
            source: RunnerSource::System,
            verified: true,
            version_string: None,
        }
    }

    fn game(runner_id: Option<Uuid>) -> Game {
        Game {
            id: Uuid::new_v4(),
            name: "test".into(),
            executable_path: PathBuf::from("/tmp/test.exe"),
            working_dir: None,
            runner_id,
            prefix_path: None,
            cover_path: None,
            platform: Platform::Windows,
            arch: Arch::Win64,
            display: Default::default(),
            env_overrides: Default::default(),
            dll_overrides: Vec::new(),
            args: Vec::new(),
            created_at: 0,
            last_played_at: None,
            playtime_seconds: 0,
        }
    }

    #[test]
    fn game_runner_wins_over_default() {
        let game_runner = runner("game");
        let default_runner = runner("default");
        let game = game(Some(game_runner.id));
        let resolved = resolve_runner(
            &game,
            &[game_runner.clone(), default_runner],
            Some(Uuid::new_v4()),
        )
        .unwrap();
        assert_eq!(resolved, game_runner);
    }

    #[test]
    fn default_runner_is_used_when_game_has_none() {
        let default_runner = runner("default");
        let game = game(None);
        let resolved = resolve_runner(
            &game,
            std::slice::from_ref(&default_runner),
            Some(default_runner.id),
        )
        .unwrap();
        assert_eq!(resolved, default_runner);
    }

    #[test]
    fn no_selection_and_no_default_is_an_error() {
        let game = game(None);
        assert!(matches!(
            resolve_runner(&game, &[runner("only")], None),
            Err(LaunchError::NoRunnerSelected)
        ));
    }

    #[test]
    fn missing_game_runner_is_reported() {
        let game = game(Some(Uuid::new_v4()));
        assert!(matches!(
            resolve_runner(&game, &[runner("other")], None),
            Err(LaunchError::RunnerNotFound(_))
        ));
    }

    #[test]
    fn missing_default_runner_is_reported() {
        let game = game(None);
        let id = Uuid::new_v4();
        assert!(matches!(
            resolve_runner(&game, &[runner("other")], Some(id)),
            Err(LaunchError::DefaultRunnerUnavailable(_))
        ));
    }
}
