use thiserror::Error;

#[derive(Debug, Error)]
pub enum DisplayError {
    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),
    #[error("unexpected DisplayConfig data: {0}")]
    Unexpected(String),
    #[error("monitor {0} is not connected")]
    MonitorNotFound(String),
    #[error("xrandr failed: {0}")]
    Xrandr(String),
}
