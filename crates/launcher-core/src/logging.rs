use std::io::Write;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Installs a stderr + daily rolling file subscriber. The returned guard must be
/// kept alive for the process lifetime so buffered log lines are flushed.
pub fn init_logging(logs_dir: &Path, default_filter: &str) -> Option<WorkerGuard> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let stderr_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true);

    match std::fs::create_dir_all(logs_dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::daily(logs_dir, "gamehub.log");
            let (file_writer, guard) = tracing_appender::non_blocking(appender);
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(file_writer);
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .with(file_layer)
                .init();
            Some(guard)
        }
        Err(error) => {
            let _ = writeln!(
                std::io::stderr(),
                "cannot create log directory {}: {error}",
                logs_dir.display()
            );
            tracing_subscriber::registry()
                .with(filter)
                .with(stderr_layer)
                .init();
            None
        }
    }
}
