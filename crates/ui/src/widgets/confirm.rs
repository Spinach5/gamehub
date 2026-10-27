use crate::app::{DeleteTarget, Message};
use iced::Element;
use iced::widget::{button, row, text};

pub fn confirm_bar(target: &DeleteTarget) -> Element<'_, Message> {
    row![
        text(format!("确认删除 {}？此操作不可撤销。", target.label()))
            .width(iced::Length::Fill)
            .wrapping(iced::widget::text::Wrapping::WordOrGlyph),
        button("确认删除").on_press(Message::DeleteConfirmed),
        button("取消").on_press(Message::CancelDelete),
    ]
    .spacing(8)
    .padding(8)
    .into()
}
