use std::io::Write;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

fn main() -> iced::Result {
    let paths = match launcher_core::AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("gamehub: cannot determine data directory: {error}");
            std::process::exit(1);
        }
    };

    let _log_guard = init_logging(&paths.logs_dir);
    tracing::info!(?paths, "gamehub starting");
    println!("gamehub data root: {}", paths.data_root.display());

    let result = run_smoke();
    tracing::info!("gamehub exiting");
    result
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

#[derive(Default)]
struct Smoke {
    clicks: u32,
    async_done: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Click,
    AsyncDone,
}

fn update(state: &mut Smoke, message: Message) -> iced::Task<Message> {
    match message {
        Message::Click => {
            state.clicks += 1;
            iced::Task::perform(
                async {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                },
                |_| Message::AsyncDone,
            )
        }
        Message::AsyncDone => {
            state.async_done = true;
            iced::Task::none()
        }
    }
}

fn view(state: &Smoke) -> iced::Element<'_, Message> {
    use iced::widget::{button, column, text};
    column![
        text("游戏启动器 中文渲染测试").size(24),
        text(format!("点击次数：{}", state.clicks)).size(18),
        text(if state.async_done {
            "tokio 异步任务：完成"
        } else {
            "tokio 异步任务：等待点击"
        }),
        button("点击测试").on_press(Message::Click),
    ]
    .spacing(12)
    .padding(24)
    .into()
}

fn run_smoke() -> iced::Result {
    iced::application("gamehub smoke", update, view)
        .theme(|_| iced::Theme::Dark)
        .window(iced::window::Settings {
            size: iced::Size::new(720.0, 480.0),
            ..Default::default()
        })
        .run_with(|| (Smoke::default(), iced::Task::none()))
}
