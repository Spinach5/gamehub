use crate::AppError;
use std::path::Path;
use std::sync::{Arc, Mutex};
use storage::Db;

/// Cheap-to-clone handle over the SQLite connection, owned by the application
/// layer so the UI never sees storage internals.
#[derive(Clone)]
pub struct AppService {
    db: Arc<Mutex<Db>>,
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
        }
    }

    pub(crate) fn db_handle(&self) -> Arc<Mutex<Db>> {
        Arc::clone(&self.db)
    }
}
