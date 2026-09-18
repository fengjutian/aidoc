//! Store = open / migrate / tx wrapper.

use std::path::Path;

use rusqlite::{Connection, Transaction};
use thiserror::Error;

use crate::schema::{DDL, SCHEMA_VERSION};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("serde_json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("timestamp parse error: {0}")]
    Timestamp(#[from] chrono::ParseError),

    #[error("integrity: {0}")]
    Integrity(String),

    #[error("closure error: {0}")]
    Closure(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl From<anyhow::Error> for StoreError {
    fn from(e: anyhow::Error) -> Self {
        struct Adaptor(anyhow::Error);
        impl std::fmt::Debug for Adaptor {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Debug::fmt(&self.0, f)
            }
        }
        impl std::fmt::Display for Adaptor {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
        impl std::error::Error for Adaptor {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                self.0.source()
            }
        }
        StoreError::Closure(Box::new(Adaptor(e)))
    }
}

/// AIDoc canonical store, holds a SQLite connection.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create + migrate) a SQLite file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        Self::initialize(conn)
    }

    /// Open an in-memory store, useful for tests.
    pub fn open_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        Self::initialize(conn)
    }

    fn initialize(conn: Connection) -> Result<Self, StoreError> {
        // Foreign keys + WAL = must be on.
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        conn.execute_batch(DDL)?;

        // Schema version bookkeeping.
        conn.execute(
            "INSERT OR IGNORE INTO schema_meta(key, value) VALUES('schema_version', ?1)",
            rusqlite::params![SCHEMA_VERSION],
        )?;

        Ok(Self { conn })
    }

    /// Borrow the raw connection. Prefer the typed CRUD methods on `Store`.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Run a closure inside a transaction. Roll back on `Err`.
    ///
    /// The closure may return *any* error; we surface it as
    /// `StoreError::Closure(Box<dyn Error>)`. This keeps `aidoc-storage`
    /// free of `aidoc-operation` (and similar) reverse dependencies.
    pub fn tx<F, T, E>(&mut self, f: F) -> Result<T, StoreError>
    where
        F: FnOnce(&Transaction<'_>) -> Result<T, E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let tx = self.conn.transaction()?;
        match f(&tx) {
            Ok(v) => {
                tx.commit()?;
                Ok(v)
            }
            Err(e) => {
                // Drop tx to roll back.
                Err(StoreError::Closure(Box::new(e)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_open_in_memory() {
        let store = Store::open_memory().expect("open");
        let n: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}