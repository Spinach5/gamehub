mod error;
mod initialize;
mod status;

#[cfg(test)]
mod test_support;

pub use error::PrefixError;
pub use status::{
    PrefixPlan, PrefixStatus, default_prefix_path, detect_arch, plan_for_game, resolve_prefix_path,
    status, windows_root,
};
