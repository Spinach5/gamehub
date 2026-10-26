use crate::app::{AppError, Message};
use chrono::{DateTime, Local};
use iced::Element;
use iced::widget::{button, column, row, text};
use std::fmt;
use std::path::Path;

pub fn section<'a>(
    title: &'a str,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    column![text(title).size(18), content.into()]
        .spacing(8)
        .padding(12)
        .into()
}

pub fn error_banner(error: &AppError) -> Element<'_, Message> {
    let mut content = column![
        row![
            text(format!("错误：{}", error.title)).size(16),
            iced::widget::horizontal_space(),
            button("关闭").on_press(Message::DismissError)
        ]
        .spacing(8)
    ]
    .spacing(4)
    .padding(8);

    if let Some(detail) = &error.detail {
        content = content.push(text(detail).size(12));
    }
    content.into()
}

pub fn fatal(message: &str) -> Element<'_, Message> {
    column![text("gamehub 无法启动").size(28), text(message).size(14),]
        .spacing(12)
        .padding(24)
        .into()
}

pub fn path_text(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "未选择".into())
}

pub fn format_timestamp(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|timestamp| DateTime::from_timestamp(timestamp, 0))
        .map(|date| {
            date.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "从未".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice<T> {
    pub value: T,
    pub label: String,
}

impl<T> Choice<T> {
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub fn arch_choices() -> Vec<Choice<launcher_core::Arch>> {
    vec![
        Choice::new(launcher_core::Arch::Win64, "64 位 (win64)"),
        Choice::new(launcher_core::Arch::Win32, "32 位 (win32)"),
    ]
}

pub fn arch_choice(arch: launcher_core::Arch) -> Choice<launcher_core::Arch> {
    match arch {
        launcher_core::Arch::Win64 => Choice::new(arch, "64 位 (win64)"),
        launcher_core::Arch::Win32 => Choice::new(arch, "32 位 (win32)"),
    }
}

pub fn confirm_bar(target: &crate::app::DeleteTarget) -> Element<'_, Message> {
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

pub fn runner_choices(
    runners: &[launcher_core::Runner],
    none_label: &str,
) -> Vec<Choice<Option<uuid::Uuid>>> {
    let mut choices = vec![Choice::new(None, none_label)];
    choices.extend(
        runners
            .iter()
            .map(|runner| Choice::new(Some(runner.id), runner.name.clone())),
    );
    choices
}

pub fn runner_choice(
    runners: &[launcher_core::Runner],
    none_label: &str,
    selected: Option<uuid::Uuid>,
) -> Choice<Option<uuid::Uuid>> {
    match selected {
        Some(id) => runners
            .iter()
            .find(|runner| runner.id == id)
            .map(|runner| Choice::new(Some(id), runner.name.clone()))
            .unwrap_or_else(|| Choice::new(None, none_label)),
        None => Choice::new(None, none_label),
    }
}
