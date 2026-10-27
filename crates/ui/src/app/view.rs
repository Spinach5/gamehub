use super::{App, DeleteTarget, Message, Screen};
use crate::theme::font;
use crate::widgets::{layout, modal};
use iced::font::Weight;
use iced::widget::{column, container, row, stack, text};
use iced::{Element, Length};

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        if let Some(fatal) = &self.fatal {
            return layout::fatal(fatal);
        }

        let screen: Element<'_, Message> = match self.screen {
            Screen::Library => crate::views::library::view(self),
            Screen::GameDetail => crate::views::game_detail::view(self),
            Screen::RunnerManager => crate::views::runners::view(self),
            Screen::Settings => crate::views::settings::view(self),
            Screen::Logs => crate::views::logs::view(self),
        };

        let mut main = column![]
            .spacing(16)
            .width(Length::Fill)
            .height(Length::Fill);
        if let Some(error) = &self.error {
            main = main.push(crate::widgets::error_banner(error));
        }
        main = main.push(screen);

        let base = container(
            row![
                sidebar(self),
                container(main)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(iced::Padding {
                        top: 20.0,
                        right: 24.0,
                        bottom: 20.0,
                        left: 24.0,
                    }),
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::canvas);

        let mut layers = stack![base];
        if let Some(target) = &self.confirming {
            layers = layers.push(modal::confirm_delete(
                delete_title(self, target),
                delete_body(target).to_owned(),
                Message::DeleteConfirmed,
                Message::CancelDelete,
            ));
        }
        layers.into()
    }
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let active = match app.screen {
        Screen::RunnerManager => Screen::RunnerManager,
        Screen::Settings => Screen::Settings,
        _ => Screen::Library,
    };

    let mark = container("")
        .width(Length::Fixed(22.0))
        .height(Length::Fixed(22.0))
        .style(|_| iced::widget::container::Style {
            background: Some(crate::theme::cover_gradient(0).into()),
            border: iced::Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let footer = layout::faint(format!(
        "{} · {}",
        session_label(app.session),
        default_runner_label(app)
    ))
    .size(10);

    container(
        column![
            row![mark, text("gameHub").font(font(Weight::Bold)).size(15)]
                .spacing(9)
                .align_y(iced::Alignment::Center),
            iced::widget::vertical_space().height(Length::Fixed(10.0)),
            layout::nav_item(
                "游戏库",
                active == Screen::Library,
                Message::Navigate(Screen::Library)
            ),
            layout::nav_item(
                "Runner 管理",
                active == Screen::RunnerManager,
                Message::Navigate(Screen::RunnerManager)
            ),
            layout::nav_item(
                "设置",
                active == Screen::Settings,
                Message::Navigate(Screen::Settings)
            ),
            iced::widget::vertical_space(),
            footer,
        ]
        .spacing(3)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([16, 12]),
    )
    .width(Length::Fixed(200.0))
    .height(Length::Fill)
    .style(crate::styles::sidebar)
    .into()
}

fn session_label(session: application::SessionType) -> &'static str {
    match session {
        application::SessionType::Wayland => "Wayland",
        application::SessionType::X11 => "X11",
        application::SessionType::Unknown => "未知会话",
    }
}

fn default_runner_label(app: &App) -> &str {
    app.settings
        .default_runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.as_str())
        .unwrap_or("未设置默认 Runner")
}

fn delete_title(app: &App, target: &DeleteTarget) -> String {
    match target {
        DeleteTarget::Game(id) => app
            .games
            .iter()
            .find(|game| game.id == *id)
            .map(|game| format!("删除游戏「{}」？", game.name))
            .unwrap_or_else(|| "删除游戏？".to_owned()),
        DeleteTarget::Runner(id) => app
            .runners
            .iter()
            .find(|runner| runner.id == *id)
            .map(|runner| format!("删除 Runner「{}」？", runner.name))
            .unwrap_or_else(|| "删除 Runner？".to_owned()),
        DeleteTarget::Prefix { path, .. } => format!("删除 Prefix {}？", path.display()),
    }
}

fn delete_body(target: &DeleteTarget) -> &'static str {
    match target {
        DeleteTarget::Game(_) => "将同时删除该游戏的配置、日志与封面，此操作不可撤销。",
        DeleteTarget::Runner(_) => "删除后需要重新导入才能使用，此操作不可撤销。",
        DeleteTarget::Prefix { .. } => "Prefix 内的游戏存档与运行环境会被删除，此操作不可撤销。",
    }
}
