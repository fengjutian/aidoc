//! Session: open a .aidoc file (extract to temp workspace + Store).

use std::path::Path;

use anyhow::{Context, Result};

use aidoc::{open_package, create_package, save_package, Package, Store};

pub struct Session {
    pub package: Option<Package>,
    pub store: Store,
}

impl Session {
    pub fn open(path: &str) -> Result<Self> {
        let (package, store) = open_package(Path::new(path)).context("open package")?;
        Ok(Self {
            package: Some(package),
            store,
        })
    }

    pub fn create(path: &str, doc_id: &str, title: &str) -> Result<Self> {
        let (package, store) =
            create_package(Path::new(path), doc_id, title).context("create package")?;
        Ok(Self {
            package: Some(package),
            store,
        })
    }

    pub fn save(mut self) -> Result<()> {
        if let Some(mut pkg) = self.package.take() {
            save_package(&mut pkg, &self.store).context("save package")?;
        }
        Ok(())
    }
}