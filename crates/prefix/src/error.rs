use launcher_core::Arch;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrefixError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("runner: {0}")]
    Runner(#[from] runner_wine::RunnerError),
    #[error("background task failed: {0}")]
    Join(String),
    #[error("prefix arch mismatch: expected {expected:?}, found {existing:?}")]
    ArchMismatch {
        expected: Arch,
        existing: Option<Arch>,
    },
    #[error("wineboot failed with code {code:?}: {stderr}")]
    WinebootFailed { code: Option<i32>, stderr: String },
    #[error("wineboot timed out after {seconds}s")]
    WinebootTimeout { seconds: u64 },
    #[error("prefix initialization finished but system.reg is missing")]
    InitializationIncomplete,
    #[error("prefix is not initialized: {0:?}")]
    PrefixNotInitialized(PathBuf),
    #[error("dxvk source is missing a required file: {path:?}")]
    DxvkSourceInvalid { path: PathBuf },
    #[error("dxvk-nvapi source is missing a required file: {path:?}")]
    DxvkNvapiSourceInvalid { path: PathBuf },
    #[error("reg add failed for {dll}: code {code:?}: {stderr}")]
    RegAddFailed {
        dll: String,
        code: Option<i32>,
        stderr: String,
    },
    #[error("reg add timed out for {dll}")]
    RegAddTimeout { dll: String },
    #[error("invalid winetricks verb: {0}")]
    WinetricksVerbInvalid(String),
    #[error("failed to start winetricks {program:?}: {source}")]
    WinetricksSpawn {
        program: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("refusing to delete a directory that is not an initialized prefix: {0:?}")]
    RefusingToDeleteNonPrefix(PathBuf),
}
