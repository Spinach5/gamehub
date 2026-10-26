use launcher_core::Arch;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("no runner selected and no default runner configured")]
    NoRunnerSelected,
    #[error("runner not found: {0}")]
    RunnerNotFound(Uuid),
    #[error("default runner not found: {0}")]
    DefaultRunnerUnavailable(Uuid),
    #[error("game executable not found: {0:?}")]
    ExecutableMissing(PathBuf),
    #[error("working directory not found: {0:?}")]
    WorkingDirMissing(PathBuf),
    #[error("runner error: {0}")]
    Runner(#[from] runner_wine::RunnerError),
    #[error("prefix is not initialized: {0:?}")]
    PrefixNotInitialized(PathBuf),
    #[error("prefix arch mismatch: expected {expected:?}, found {existing:?}")]
    PrefixArchMismatch {
        expected: Arch,
        existing: Option<Arch>,
    },
    #[error("gamescope is forced but not available")]
    GamescopeUnavailable,
    #[error("WINEDLLOVERRIDES must be configured through structured dll_overrides")]
    DllOverrideConflict,
    #[error("invalid DLL name: {0:?}")]
    InvalidDllName(String),
    #[error("invalid environment variable key: {0:?}")]
    InvalidEnvKey(String),
    #[error("failed to spawn {program:?}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
}
