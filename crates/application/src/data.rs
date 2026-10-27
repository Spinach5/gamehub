use crate::{AppError, AppService};
use display::{SessionType, X11Output};
use launcher_core::{Game, Runner};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LoadedData {
    pub games: Vec<Game>,
    pub runners: Vec<Runner>,
    pub settings_map: BTreeMap<String, String>,
    pub session: SessionType,
    pub x11_outputs: Vec<X11Output>,
    pub gamescope_path: Option<PathBuf>,
}

impl AppService {
    pub async fn load_initial(self) -> Result<LoadedData, AppError> {
        let db = self.db_handle();
        tokio::task::spawn_blocking(move || {
            let db = db.lock().map_err(|_| AppError::DatabaseLocked)?;
            Ok(LoadedData {
                games: db.list_games()?,
                runners: db.list_runners()?,
                settings_map: db.get_all_settings()?,
                session: display::detect_session(),
                x11_outputs: display::list_x11_outputs(),
                gamescope_path: display::gamescope_path(),
            })
        })
        .await?
    }
}
