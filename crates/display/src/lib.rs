mod session;
mod xrandr;

pub use session::{SessionType, detect_session, session_from};
pub use xrandr::{X11Output, list_x11_outputs, parse_xrandr_output};
