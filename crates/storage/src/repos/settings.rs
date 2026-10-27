use crate::{Db, StorageError};
use rusqlite::OptionalExtension;
use rusqlite::params;
use std::collections::BTreeMap;

impl Db {
    pub fn get_setting(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    pub fn get_all_settings(&self) -> Result<BTreeMap<String, String>, StorageError> {
        let mut statement = self
            .conn
            .prepare("SELECT key, value FROM settings ORDER BY key")?;
        let mut rows = statement.query([])?;
        let mut settings = BTreeMap::new();
        while let Some(row) = rows.next()? {
            settings.insert(row.get(0)?, row.get(1)?);
        }
        Ok(settings)
    }
}
