mod error;
mod gamescope;
mod primary;
mod session;
mod xrandr;

pub use error::DisplayError;
pub use gamescope::{
    build_gamescope_args, find_in_path, gamescope_path, mangohud_path, should_use_gamescope,
};
pub use primary::set_primary_output;
pub use session::{SessionType, detect_session, session_from};
pub use xrandr::{MonitorOutput, list_outputs, parse_wayland_info_outputs, parse_xrandr_output};
