mod context;
mod detect;
mod error;
mod status;
mod stop;

pub use context::WineContext;
pub use detect::{
    candidate_wine_paths, candidate_wine_paths_with, detect_from_candidates, detect_system_wine,
    validate_proton_dir, verify_runner,
};
pub use error::RunnerError;
pub use status::prefix_has_processes;
pub use stop::{kill_wine_prefix, wineserver_path};

#[cfg(test)]
pub(crate) mod test_support {
    use tokio::sync::Mutex;

    /// Executing a freshly written script while a concurrent fork still holds
    /// its write fd yields ETXTBSY; tests that spawn scripts serialize here.
    pub(crate) static EXEC_LOCK: Mutex<()> = Mutex::const_new(());
}
