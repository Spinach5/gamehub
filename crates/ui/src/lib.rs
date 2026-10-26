pub mod app;
pub mod dialogs;
pub mod draft;
pub mod tasks;
pub mod views;
pub mod widgets;

pub use app::{App, Message};

use launcher_core::AppPaths;
use std::io::Write;
use std::path::Path;
use storage::Db;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn run() -> iced::Result {
    let paths = match AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("gamehub: cannot determine data directory: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = storage::ensure_app_dirs(&paths) {
        eprintln!("gamehub: cannot create application directories: {error}");
    }

    let _log_guard = init_logging(&paths.logs_dir);
    tracing::info!(?paths, "gamehub starting");

    let db = Db::open(&paths.db_path);

    iced::application("gamehub", App::update, App::view)
        .theme(|_| iced::Theme::Dark)
        .default_font(iced::Font::with_name("Noto Sans CJK SC"))
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 760.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            ..Default::default()
        })
        .run_with(move || App::new(paths, db))
}

fn init_logging(logs_dir: &Path) -> Option<WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ui=debug,launch=debug,storage=debug"));

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
                "gamehub: cannot create log directory {}: {error}",
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
