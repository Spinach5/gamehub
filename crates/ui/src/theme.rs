use iced::font::{Family, Weight};
use iced::theme::Palette;
use iced::{Color, Font, Theme};

pub const FONT_FAMILY: &str = "Noto Sans CJK SC";

/// Noto Sans CJK ships only Regular and Bold faces. An intermediate weight
/// makes iced fall back to a face without CJK coverage, which draws Chinese
/// as nothing at all, so every request snaps to one of the two real faces.
fn shipped_weight(weight: Weight) -> Weight {
    if matches!(
        weight,
        Weight::Semibold | Weight::Bold | Weight::ExtraBold | Weight::Black
    ) {
        Weight::Bold
    } else {
        Weight::Normal
    }
}

pub fn font(weight: Weight) -> Font {
    Font {
        family: Family::Name(FONT_FAMILY),
        weight: shipped_weight(weight),
        ..Font::DEFAULT
    }
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb(
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    )
}

const fn rgba(hex: u32, alpha: f32) -> Color {
    let color = rgb(hex);
    Color {
        r: color.r,
        g: color.g,
        b: color.b,
        a: alpha,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

impl ThemeMode {
    pub fn from_setting(value: Option<&str>) -> Self {
        match value {
            Some("dark") => Self::Dark,
            _ => Self::Light,
        }
    }

    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn label(self, lang: crate::i18n::Lang) -> &'static str {
        match self {
            Self::Light => crate::i18n::tr(lang, "settings.theme.light"),
            Self::Dark => crate::i18n::tr(lang, "settings.theme.dark"),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Light => "gameHub Light",
            Self::Dark => "gameHub Dark",
        }
    }

    fn palette(self) -> Palette {
        match self {
            Self::Light => Palette {
                background: rgb(0xf4f5fb),
                text: rgb(0x171a2b),
                primary: rgb(0x5b5bd6),
                success: rgb(0x0f9f6e),
                danger: rgb(0xe11d48),
            },
            Self::Dark => Palette {
                background: rgb(0x10121a),
                text: rgb(0xe8eaf3),
                primary: rgb(0x6060e0),
                success: rgb(0x34d399),
                danger: rgb(0xfb7185),
            },
        }
    }

    pub fn theme(self) -> Theme {
        Theme::custom(self.name().to_owned(), self.palette())
    }
}

pub struct Tokens {
    pub canvas: Color,
    pub sidebar: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub border: Color,
    pub border_strong: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_faint: Color,
    pub primary: Color,
    pub primary_hover: Color,
    pub primary_soft: Color,
    pub primary_text: Color,
    pub on_primary: Color,
    pub success: Color,
    pub success_soft: Color,
    pub warning: Color,
    pub warning_soft: Color,
    pub danger: Color,
    pub danger_soft: Color,
    pub shadow: Color,
    pub backdrop: Color,
    pub log_surface: Color,
    pub log_text: Color,
}

pub static LIGHT: Tokens = Tokens {
    canvas: rgb(0xf4f5fb),
    sidebar: rgb(0xffffff),
    surface: rgb(0xffffff),
    surface_alt: rgb(0xf8f9fd),
    border: rgb(0xe6e8f2),
    border_strong: rgb(0xd9dcec),
    text: rgb(0x171a2b),
    text_muted: rgb(0x5f6678),
    text_faint: rgb(0x676e85),
    primary: rgb(0x5b5bd6),
    primary_hover: rgb(0x4f4fc4),
    primary_soft: rgb(0xecebff),
    primary_text: rgb(0x4f46c9),
    on_primary: rgb(0xffffff),
    success: rgb(0x0a7a55),
    success_soft: rgb(0xe8f9f1),
    warning: rgb(0xb45309),
    warning_soft: rgb(0xfef3c7),
    danger: rgb(0xd11a43),
    danger_soft: rgb(0xfff1f2),
    shadow: rgba(0x171a2b, 0.07),
    backdrop: rgba(0x171a2b, 0.45),
    log_surface: rgb(0x0f1117),
    log_text: rgb(0xd7dbe8),
};

pub static DARK: Tokens = Tokens {
    canvas: rgb(0x10121a),
    sidebar: rgb(0x151822),
    surface: rgb(0x191c28),
    surface_alt: rgb(0x1e2230),
    border: rgb(0x262b3a),
    border_strong: rgb(0x333a4d),
    text: rgb(0xe8eaf3),
    text_muted: rgb(0x9aa1b8),
    text_faint: rgb(0x848a9e),
    primary: rgb(0x6060e0),
    primary_hover: rgb(0x5a5ad8),
    primary_soft: rgba(0x6060e0, 0.18),
    primary_text: rgb(0xb9b9ff),
    on_primary: rgb(0xffffff),
    success: rgb(0x34d399),
    success_soft: rgba(0x34d399, 0.13),
    warning: rgb(0xfbbf24),
    warning_soft: rgba(0xfbbf24, 0.14),
    danger: rgb(0xfb7185),
    danger_soft: rgba(0xfb7185, 0.13),
    shadow: rgba(0x000000, 0.5),
    backdrop: rgba(0x000000, 0.6),
    log_surface: rgb(0x0b0d13),
    log_text: rgb(0xc9cedd),
};

pub fn tokens(theme: &Theme) -> &'static Tokens {
    if theme.extended_palette().is_dark {
        &DARK
    } else {
        &LIGHT
    }
}

const COVER_GRADIENTS: [(u32, u32); 6] = [
    (0x6366f1, 0xa855f7),
    (0xf59e0b, 0xf43f5e),
    (0x10b981, 0x0ea5e9),
    (0x8b5cf6, 0x1e3a8a),
    (0xf43f5e, 0x7c2d12),
    (0x0ea5e9, 0x155e75),
];

pub fn cover_gradient(seed: u128) -> iced::gradient::Linear {
    let (start, end) = COVER_GRADIENTS[(seed % COVER_GRADIENTS.len() as u128) as usize];
    iced::gradient::Linear::new(135.0_f32.to_radians())
        .add_stop(0.0, rgb(start))
        .add_stop(1.0, rgb(end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_parses_known_values_and_falls_back_to_light() {
        assert_eq!(ThemeMode::from_setting(None), ThemeMode::Light);
        assert_eq!(ThemeMode::from_setting(Some("dark")), ThemeMode::Dark);
        assert_eq!(ThemeMode::from_setting(Some("light")), ThemeMode::Light);
        assert_eq!(ThemeMode::from_setting(Some("whatever")), ThemeMode::Light);
    }

    #[test]
    fn theme_palette_brightness_matches_mode() {
        assert!(!ThemeMode::Light.theme().extended_palette().is_dark);
        assert!(ThemeMode::Dark.theme().extended_palette().is_dark);
    }

    #[test]
    fn tokens_select_the_matching_table() {
        assert!(std::ptr::eq(tokens(&ThemeMode::Light.theme()), &LIGHT));
        assert!(std::ptr::eq(tokens(&ThemeMode::Dark.theme()), &DARK));
    }

    #[test]
    fn font_weights_snap_to_the_shipped_faces() {
        for weight in [
            Weight::Semibold,
            Weight::Bold,
            Weight::ExtraBold,
            Weight::Black,
        ] {
            assert_eq!(font(weight).weight, Weight::Bold, "{weight:?}");
        }
        for weight in [Weight::Thin, Weight::Light, Weight::Normal, Weight::Medium] {
            assert_eq!(font(weight).weight, Weight::Normal, "{weight:?}");
        }
        assert!(matches!(
            font(Weight::Semibold).family,
            Family::Name(FONT_FAMILY)
        ));
    }
}
