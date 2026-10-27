use crate::app::Message;
use crate::theme;
use iced::font::Weight;
use iced::widget::image;
use iced::{Border, Color, ContentFit, Element, Length};
use uuid::Uuid;

pub fn cover<'a>(
    name: &str,
    id: Uuid,
    handle: Option<image::Handle>,
    width: f32,
    height: f32,
) -> Element<'a, Message> {
    let initial = name
        .trim()
        .chars()
        .next()
        .map(|character| character.to_string())
        .unwrap_or_else(|| "?".to_owned());
    let font_size = (width.min(height) * 0.34).clamp(14.0, 42.0);
    let gradient = theme::cover_gradient(id.as_u128());

    let placeholder = iced::widget::container(
        iced::widget::text(initial)
            .size(font_size)
            .font(theme::font(Weight::Bold))
            .color(Color::WHITE),
    )
    .width(Length::Fixed(width))
    .height(Length::Fixed(height))
    .align_x(iced::Alignment::Center)
    .align_y(iced::Alignment::Center)
    .style(move |_| iced::widget::container::Style {
        background: Some(gradient.into()),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let mut layers = iced::widget::Stack::new().push(placeholder);
    if let Some(handle) = handle {
        layers = layers.push(
            iced::widget::image(handle)
                .width(Length::Fixed(width))
                .height(Length::Fixed(height))
                .content_fit(ContentFit::Cover),
        );
    }

    layers
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .into()
}
