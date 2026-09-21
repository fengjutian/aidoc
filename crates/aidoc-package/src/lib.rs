//! AIDoc package: ZIP container with canonical JSON, schemas, derived HTML,
//! assets, and an optional SQLite implementation store.

pub mod manifest;
pub mod workspace;
pub mod zip_io;

pub use manifest::Manifest;
pub use workspace::{
    Package, PackageError, create_package, inline_image_assets, open_package, save_package,
};
