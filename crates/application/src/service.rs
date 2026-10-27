use crate::AppError;
use launcher_core::Runner;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use storage::Db;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct RunningGame {
    pub process_group: u32,
    pub runner: Runner,
    pub wine_prefix: PathBuf,
}

/// Cheap-to-clone handle over the SQLite connection, owned by the application
/// layer so the UI never sees storage internals.
#[derive(Clone)]
pub struct AppService {
    db: Arc<Mutex<Db>>,
    running_games: Arc<Mutex<HashMap<Uuid, RunningGame>>>,
    stop_requested: Arc<Mutex<HashSet<Uuid>>>,
}

impl AppService {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        Ok(Self::new(Db::open(path)?))
    }

    pub fn open_in_memory() -> Result<Self, AppError> {
        Ok(Self::new(Db::open_in_memory()?))
    }

    pub fn new(db: Db) -> Self {
        Self {
            db: Arc::new(Mutex::new(db)),
            running_games: Arc::new(Mutex::new(HashMap::new())),
            stop_requested: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(crate) fn db_handle(&self) -> Arc<Mutex<Db>> {
        Arc::clone(&self.db)
    }

    pub(crate) fn register_process(
        &self,
        game_id: Uuid,
        process_group: u32,
        runner: Runner,
        wine_prefix: PathBuf,
    ) -> bool {
        self.lock_running_games()
            .insert(
                game_id,
                RunningGame {
                    process_group,
                    runner,
                    wine_prefix,
                },
            )
            .is_none()
    }

    pub(crate) fn unregister_process(&self, game_id: Uuid) {
        self.lock_running_games().remove(&game_id);
    }

    pub(crate) fn running_game(&self, game_id: Uuid) -> Option<RunningGame> {
        self.lock_running_games().get(&game_id).cloned()
    }

    pub(crate) fn mark_stop_requested(&self, game_id: Uuid) {
        self.lock_stop_requested().insert(game_id);
    }

    pub(crate) fn clear_stop_requested(&self, game_id: Uuid) {
        self.lock_stop_requested().remove(&game_id);
    }

    pub(crate) fn take_stop_requested(&self, game_id: Uuid) -> bool {
        self.lock_stop_requested().remove(&game_id)
    }

    fn lock_running_games(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, RunningGame>> {
        self.running_games
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_stop_requested(&self) -> std::sync::MutexGuard<'_, HashSet<Uuid>> {
        self.stop_requested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
