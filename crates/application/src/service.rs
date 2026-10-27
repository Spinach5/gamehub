use crate::AppError;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};
use storage::Db;
use uuid::Uuid;

/// Cheap-to-clone handle over the SQLite connection, owned by the application
/// layer so the UI never sees storage internals.
#[derive(Clone)]
pub struct AppService {
    db: Arc<Mutex<Db>>,
    process_groups: Arc<Mutex<HashMap<Uuid, u32>>>,
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
            process_groups: Arc::new(Mutex::new(HashMap::new())),
            stop_requested: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub(crate) fn db_handle(&self) -> Arc<Mutex<Db>> {
        Arc::clone(&self.db)
    }

    pub(crate) fn register_process(&self, game_id: Uuid, process_group: u32) -> bool {
        let mut groups = self.lock_process_groups();
        groups.insert(game_id, process_group).is_none()
    }

    pub(crate) fn unregister_process(&self, game_id: Uuid) {
        self.lock_process_groups().remove(&game_id);
    }

    pub(crate) fn process_group(&self, game_id: Uuid) -> Option<u32> {
        self.lock_process_groups().get(&game_id).copied()
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

    fn lock_process_groups(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, u32>> {
        self.process_groups
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn lock_stop_requested(&self) -> std::sync::MutexGuard<'_, HashSet<Uuid>> {
        self.stop_requested
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
