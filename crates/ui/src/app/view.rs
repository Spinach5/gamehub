use super::{App, Message, Screen};
use iced::Element;

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        if let Some(fatal) = &self.fatal {
            return crate::widgets::fatal(fatal);
        }

        let screen: Element<'_, Message> = match self.screen {
            Screen::Library => crate::views::library::view(self),
            Screen::GameDetail => crate::views::game_detail::view(self),
            Screen::RunnerManager => crate::views::runners::view(self),
            Screen::Settings => crate::views::settings::view(self),
        };

        let mut content = match &self.error {
            Some(error) => iced::widget::column![crate::widgets::error_banner(error), screen]
                .spacing(8)
                .padding(8)
                .into(),
            None => screen,
        };

        if let Some(target) = &self.confirming {
            content = iced::widget::column![crate::widgets::confirm_bar(target), content]
                .spacing(8)
                .padding(8)
                .into();
        }

        content
    }
}
