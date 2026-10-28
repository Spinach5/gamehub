pub mod app;
pub mod dialogs;
pub mod draft;
pub mod i18n;
pub mod styles;
pub mod theme;
pub mod views;
pub mod widgets;

pub use app::{App, Message};

use application::{AppPaths, AppService};

pub fn run() -> iced::Result {
    let paths = match AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("gamehub: cannot determine data directory: {error}");
            std::process::exit(1);
        }
    };

    if let Err(error) = application::ensure_app_dirs(&paths) {
        eprintln!("gamehub: cannot create application directories: {error}");
    }

    let _log_guard = launcher_core::init_logging(
        &paths.logs_dir,
        "info,ui=debug,application=debug,launch=debug,storage=debug",
    );
    tracing::info!(?paths, "gamehub starting");

    let service = AppService::open(&paths.db_path);

    iced::application("gamehub", App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .default_font(iced::Font::with_name("Noto Sans CJK SC"))
        .window(iced::window::Settings {
            size: iced::Size::new(1180.0, 760.0),
            min_size: Some(iced::Size::new(900.0, 600.0)),
            ..Default::default()
        })
        .run_with(move || App::new(paths, service))
}
