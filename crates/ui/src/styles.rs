use crate::theme::tokens;
use iced::overlay::menu;
use iced::widget::{
    button, checkbox as checkbox_widget, container, pick_list as pick_list_widget,
    scrollable as scrollable_widget, text_input as text_input_widget, toggler as toggler_widget,
};
use iced::{Background, Border, Color, Shadow, Theme, Vector};

const RADIUS_CARD: f32 = 14.0;
const RADIUS_CONTROL: f32 = 8.0;

fn border(color: Color, width: f32, radius: f32) -> Border {
    Border {
        color,
        width,
        radius: radius.into(),
    }
}

fn shadow(color: Color, y: f32, blur: f32) -> Shadow {
    Shadow {
        color,
        offset: Vector::new(0.0, y),
        blur_radius: blur,
    }
}

fn button_style(
    background: Option<Background>,
    text_color: Color,
    border_style: Border,
) -> button::Style {
    button::Style {
        background,
        text_color,
        border: border_style,
        shadow: Shadow::default(),
    }
}

pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Active => button_style(
            Some(t.primary.into()),
            t.on_primary,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.primary_hover.into()),
            t.on_primary,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
        button::Status::Disabled => button_style(
            Some(t.surface_alt.into()),
            t.text_faint,
            border(t.border, 1.0, RADIUS_CONTROL),
        ),
    }
}

pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Active => button_style(
            Some(t.surface.into()),
            t.text,
            border(t.border_strong, 1.0, RADIUS_CONTROL),
        ),
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.surface_alt.into()),
            t.text,
            border(t.border_strong, 1.0, RADIUS_CONTROL),
        ),
        button::Status::Disabled => button_style(
            Some(t.surface_alt.into()),
            t.text_faint,
            border(t.border, 1.0, RADIUS_CONTROL),
        ),
    }
}

pub fn danger(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Disabled => button_style(
            Some(t.surface_alt.into()),
            t.text_faint,
            border(t.border, 1.0, RADIUS_CONTROL),
        ),
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.danger_soft.into()),
            t.danger,
            border(t.danger, 1.0, RADIUS_CONTROL),
        ),
        button::Status::Active => button_style(
            Some(t.danger_soft.into()),
            t.danger,
            border(t.danger, 0.0, RADIUS_CONTROL),
        ),
    }
}

pub fn quiet(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Active => button_style(
            None,
            t.text_muted,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.surface_alt.into()),
            t.text,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
        button::Status::Disabled => button_style(
            None,
            t.text_faint,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
    }
}

pub fn nav_item(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.surface_alt.into()),
            t.text,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
        _ => button_style(
            None,
            t.text_muted,
            border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
        ),
    }
}

pub fn nav_active(theme: &Theme, _status: button::Status) -> button::Style {
    let t = tokens(theme);
    button_style(
        Some(t.primary_soft.into()),
        t.primary_text,
        border(Color::TRANSPARENT, 0.0, RADIUS_CONTROL),
    )
}

pub fn segment(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    match status {
        button::Status::Hovered | button::Status::Pressed => button_style(
            Some(t.surface.into()),
            t.text,
            border(Color::TRANSPARENT, 0.0, 7.0),
        ),
        _ => button_style(None, t.text_muted, border(Color::TRANSPARENT, 0.0, 7.0)),
    }
}

pub fn segment_selected(theme: &Theme, _status: button::Status) -> button::Style {
    let t = tokens(theme);
    button::Style {
        background: Some(t.surface.into()),
        text_color: t.primary_text,
        border: border(Color::TRANSPARENT, 0.0, 7.0),
        shadow: shadow(t.shadow, 1.0, 4.0),
    }
}

pub fn link(theme: &Theme, status: button::Status) -> button::Style {
    let t = tokens(theme);
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => t.primary_text,
        _ => t.text_faint,
    };
    button_style(None, text_color, Border::default())
}

pub fn list_panel(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface_alt.into()),
        border: border(t.border, 1.0, 10.0),
        ..Default::default()
    }
}

pub fn segment_track(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface_alt.into()),
        border: border(Color::TRANSPARENT, 0.0, 9.0),
        ..Default::default()
    }
}

pub fn canvas(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(theme).canvas.into()),
        ..Default::default()
    }
}

pub fn sidebar(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.sidebar.into()),
        border: Border {
            color: t.border,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

pub fn card(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface.into()),
        border: border(t.border, 1.0, RADIUS_CARD),
        shadow: shadow(t.shadow, 2.0, 8.0),
        ..Default::default()
    }
}

pub fn modal_card(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.surface.into()),
        border: border(t.border, 1.0, 16.0),
        shadow: shadow(t.shadow, 12.0, 32.0),
        ..Default::default()
    }
}

pub fn backdrop(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(tokens(theme).backdrop.into()),
        ..Default::default()
    }
}

pub fn log_panel(theme: &Theme) -> container::Style {
    let t = tokens(theme);
    container::Style {
        background: Some(t.log_surface.into()),
        border: border(t.border, 1.0, RADIUS_CARD),
        ..Default::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Live,
    Starting,
    Warning,
    Danger,
    Neutral,
    Info,
}

pub fn tone_colors(theme: &Theme, tone: Tone) -> (Color, Color) {
    let t = tokens(theme);
    match tone {
        Tone::Live => (t.success_soft, t.success),
        Tone::Starting => (t.warning_soft, t.warning),
        Tone::Warning => (t.warning_soft, t.warning),
        Tone::Danger => (t.danger_soft, t.danger),
        Tone::Neutral => (t.surface_alt, t.text_muted),
        Tone::Info => (t.primary_soft, t.primary_text),
    }
}

pub fn badge(theme: &Theme, tone: Tone) -> container::Style {
    let (background, _) = tone_colors(theme, tone);
    container::Style {
        background: Some(background.into()),
        border: Border {
            radius: 999.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn text_input(theme: &Theme, status: text_input_widget::Status) -> text_input_widget::Style {
    let t = tokens(theme);
    let (background, border_color, width) = match status {
        text_input_widget::Status::Active => (t.surface_alt, t.border, 1.0),
        text_input_widget::Status::Hovered => (t.surface_alt, t.border_strong, 1.0),
        text_input_widget::Status::Focused => (t.surface, t.primary, 1.5),
        text_input_widget::Status::Disabled => (t.surface_alt, t.border, 1.0),
    };
    text_input_widget::Style {
        background: background.into(),
        border: border(border_color, width, RADIUS_CONTROL),
        icon: t.text_muted,
        placeholder: t.text_faint,
        value: t.text,
        selection: t.primary_soft,
    }
}

pub fn pick_list(theme: &Theme, status: pick_list_widget::Status) -> pick_list_widget::Style {
    let t = tokens(theme);
    let (background, border_color) = match status {
        pick_list_widget::Status::Active => (t.surface_alt, t.border),
        pick_list_widget::Status::Hovered => (t.surface_alt, t.border_strong),
        pick_list_widget::Status::Opened => (t.surface, t.primary),
    };
    pick_list_widget::Style {
        text_color: t.text,
        placeholder_color: t.text_faint,
        handle_color: t.text_muted,
        background: background.into(),
        border: border(border_color, 1.0, RADIUS_CONTROL),
    }
}

pub fn menu(theme: &Theme) -> menu::Style {
    let t = tokens(theme);
    menu::Style {
        background: t.surface.into(),
        border: border(t.border, 1.0, RADIUS_CONTROL),
        text_color: t.text,
        selected_text_color: t.primary_text,
        selected_background: t.primary_soft.into(),
    }
}

pub fn toggler(theme: &Theme, status: toggler_widget::Status) -> toggler_widget::Style {
    let t = tokens(theme);
    let is_toggled = match status {
        toggler_widget::Status::Active { is_toggled }
        | toggler_widget::Status::Hovered { is_toggled } => is_toggled,
        toggler_widget::Status::Disabled => false,
    };
    toggler_widget::Style {
        background: if is_toggled {
            t.primary
        } else {
            t.border_strong
        },
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Color::WHITE,
        foreground_border_width: 1.0,
        foreground_border_color: t.border_strong,
    }
}

pub fn checkbox(theme: &Theme, status: checkbox_widget::Status) -> checkbox_widget::Style {
    let t = tokens(theme);
    let (is_checked, is_hovered, is_disabled) = match status {
        checkbox_widget::Status::Active { is_checked } => (is_checked, false, false),
        checkbox_widget::Status::Hovered { is_checked } => (is_checked, true, false),
        checkbox_widget::Status::Disabled { is_checked } => (is_checked, false, true),
    };

    let (background, border_color, icon_color) = if is_disabled {
        (
            t.surface_alt,
            t.border,
            if is_checked {
                t.text_faint
            } else {
                Color::TRANSPARENT
            },
        )
    } else if is_checked {
        (t.primary, t.primary, t.on_primary)
    } else {
        (
            t.surface,
            if is_hovered {
                t.border_strong
            } else {
                t.border
            },
            Color::TRANSPARENT,
        )
    };

    checkbox_widget::Style {
        background: background.into(),
        icon_color,
        border: border(border_color, 1.0, 5.0),
        text_color: Some(if is_disabled { t.text_faint } else { t.text }),
    }
}

pub fn scrollable(theme: &Theme, _status: scrollable_widget::Status) -> scrollable_widget::Style {
    let t = tokens(theme);
    let rail = scrollable_widget::Rail {
        background: Some(t.surface_alt.into()),
        border: Border::default(),
        scroller: scrollable_widget::Scroller {
            color: t.border_strong,
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
        },
    };
    scrollable_widget::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}

pub fn log_scrollable(
    theme: &Theme,
    _status: scrollable_widget::Status,
) -> scrollable_widget::Style {
    let t = tokens(theme);
    let rail = scrollable_widget::Rail {
        background: Some(t.log_surface.into()),
        border: Border::default(),
        scroller: scrollable_widget::Scroller {
            color: t.log_text.scale_alpha(0.45),
            border: Border {
                radius: 999.0.into(),
                ..Default::default()
            },
        },
    };
    scrollable_widget::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
    }
}
