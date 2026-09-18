//! SQLite canonical store for AIDoc (spec §32).
//!
//! Owns the connection lifecycle, schema migrations, and a transaction
//! wrapper. Higher crates talk to the typed CRUD methods, never to raw SQL.

pub mod crud;
pub mod schema;
pub mod store;

pub use store::{Store, StoreError};

pub use crud::{FullSnapshot, SnapshotStrategy};

pub use rusqlite;

/// Adapter so `anyhow::Error` (which doesn't impl `std::error::Error`) can
/// be returned from `Store::tx` closures.
pub struct AnyhowErr(pub anyhow::Error);

impl std::fmt::Debug for AnyhowErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}
impl std::fmt::Display for AnyhowErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl std::error::Error for AnyhowErr {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<anyhow::Error> for AnyhowErr {
    fn from(e: anyhow::Error) -> Self {
        Self(e)
    }
}

impl From<StoreError> for AnyhowErr {
    fn from(e: StoreError) -> Self {
        Self(anyhow::Error::new(e))
    }
}
