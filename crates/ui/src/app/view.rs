use super::{App, DeleteTarget, Message, Screen};
use crate::theme::font;
use crate::widgets::{layout, modal};
use iced::font::Weight;
use iced::widget::{button, column, container, row, stack, text, tooltip};
use iced::{Element, Length};

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        if let Some(fatal) = &self.fatal {
            return layout::fatal(self.lang, fatal);
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
            main = main.push(crate::widgets::error_banner(self.lang, error));
        }
        if let Some(success) = &self.success {
            main = main.push(crate::widgets::success_banner(self.lang, success));
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
        if self.new_game_platform.is_some() {
            layers = layers.push(modal::platform_prompt(
                self.lang,
                Message::BeginNewGame(application::Platform::Windows),
                Message::BeginNewGame(application::Platform::Linux),
                Message::CancelNewGame,
            ));
        }
        if let Some(target) = &self.confirming {
            layers = layers.push(modal::confirm_delete(
                self.lang,
                delete_title(self, target),
                delete_body(self.lang, target).to_owned(),
                Message::DeleteConfirmed,
                Message::CancelDelete,
            ));
        }
        if let Some(dialog) = &self.dll_scan {
            layers = layers.push(crate::views::dll_scan::view(self.lang, dialog));
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

    let mark = iced::widget::svg(iced::widget::svg::Handle::from_memory(
        include_bytes!("../../../../packaging/gamehub.svg").as_slice(),
    ))
    .width(Length::Fixed(22.0))
    .height(Length::Fixed(22.0));

    let footer = column![
        layout::faint(format!(
            "{} · {}",
            session_label(app.lang, app.session),
            default_runner_label(app)
        ))
        .size(10),
        tooltip(
            button(
                text(format!("v{}", env!("CARGO_PKG_VERSION")))
                    .size(10)
                    .font(font(Weight::Normal)),
            )
            .padding([3, 0])
            .style(crate::styles::link)
            .on_press(Message::OpenHomepage),
            text(app.tr("app.version.tooltip")).size(12),
            tooltip::Position::Top,
        )
        .gap(4),
    ]
    .spacing(2);

    container(
        column![
            row![mark, text("gameHub").font(font(Weight::Bold)).size(15)]
                .spacing(9)
                .align_y(iced::Alignment::Center),
            iced::widget::vertical_space().height(Length::Fixed(10.0)),
            layout::nav_item(
                app.tr("nav.library"),
                active == Screen::Library,
                Message::Navigate(Screen::Library)
            ),
            layout::nav_item(
                app.tr("nav.runners"),
                active == Screen::RunnerManager,
                Message::Navigate(Screen::RunnerManager)
            ),
            layout::nav_item(
                app.tr("nav.settings"),
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

fn session_label(lang: crate::i18n::Lang, session: application::SessionType) -> &'static str {
    match session {
        application::SessionType::Wayland => "Wayland",
        application::SessionType::X11 => "X11",
        application::SessionType::Unknown => crate::i18n::tr(lang, "app.session.unknown"),
    }
}

fn default_runner_label(app: &App) -> &str {
    app.settings
        .default_runner_id
        .and_then(|id| app.runners.iter().find(|runner| runner.id == id))
        .map(|runner| runner.name.as_str())
        .unwrap_or_else(|| app.tr("launch.no_default_runner"))
}

fn delete_title(app: &App, target: &DeleteTarget) -> String {
    match target {
        DeleteTarget::Game(id) => app
            .games
            .iter()
            .find(|game| game.id == *id)
            .map(|game| app.tr_args("delete.title.game", &[("name", &game.name)]))
            .unwrap_or_else(|| app.tr("delete.title.game_fallback").to_owned()),
        DeleteTarget::Runner(id) => app
            .runners
            .iter()
            .find(|runner| runner.id == *id)
            .map(|runner| app.tr_args("delete.title.runner", &[("name", &runner.name)]))
            .unwrap_or_else(|| app.tr("delete.title.runner_fallback").to_owned()),
        DeleteTarget::Prefix { path, .. } => app.tr_args(
            "delete.title.prefix",
            &[("path", &path.display().to_string())],
        ),
    }
}

fn delete_body(lang: crate::i18n::Lang, target: &DeleteTarget) -> &'static str {
    match target {
        DeleteTarget::Game(_) => crate::i18n::tr(lang, "delete.body.game"),
        DeleteTarget::Runner(_) => crate::i18n::tr(lang, "delete.body.runner"),
        DeleteTarget::Prefix { .. } => crate::i18n::tr(lang, "delete.body.prefix"),
    }
}
