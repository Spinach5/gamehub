use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppPathsError {
    #[error("neither XDG_DATA_HOME nor HOME is set")]
    NoDataDir,
}
