#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionType {
    X11,
    Wayland,
    Unknown,
}

pub fn session_from(value: Option<&str>) -> SessionType {
    match value {
        Some("x11") => SessionType::X11,
        Some("wayland") => SessionType::Wayland,
        _ => SessionType::Unknown,
    }
}

pub fn detect_session() -> SessionType {
    session_from(std::env::var("XDG_SESSION_TYPE").ok().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_x11_and_wayland_exactly() {
        assert_eq!(session_from(Some("x11")), SessionType::X11);
        assert_eq!(session_from(Some("wayland")), SessionType::Wayland);
    }

    #[test]
    fn unknown_and_missing_are_unknown() {
        assert_eq!(session_from(None), SessionType::Unknown);
        assert_eq!(session_from(Some("")), SessionType::Unknown);
        assert_eq!(session_from(Some("X11")), SessionType::Unknown);
        assert_eq!(session_from(Some("tty")), SessionType::Unknown);
    }
}
