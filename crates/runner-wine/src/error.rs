use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("proton directory is invalid (proton script missing or not executable): {0:?}")]
    InvalidProtonDir(PathBuf),
    #[error("runner binary is missing: {0:?}")]
    MissingBinary(PathBuf),
    #[error("runner binary is not executable: {0:?}")]
    NotExecutable(PathBuf),
    #[error("cannot canonicalize proton directory {path:?}: {source}")]
    Canonicalize {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("wine version probe failed for {path:?} with code {code:?}: {stderr}")]
    ProbeFailed {
        path: PathBuf,
        code: Option<i32>,
        stderr: String,
    },
    #[error("wine version probe timed out for {0:?}")]
    ProbeTimeout(PathBuf),
    #[error("failed to spawn {program:?}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}
