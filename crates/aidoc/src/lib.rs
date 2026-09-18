//! AIDoc facade crate.
//!
//! Re-exports the public surface of every core crate so callers can simply
//! `use aidoc::*` and skip per-crate paths.

pub use aidoc_model as model;
pub use aidoc_model::*;
pub use aidoc_storage as storage;
pub use aidoc_storage::{crud, AnyhowErr, Store, StoreError};
pub use aidoc_operation as operation;
pub use aidoc_operation::{apply_operation, ApplyError, ApplyOutcome, OperationError};
pub use aidoc_history as history;
pub use aidoc_history::{revert_to, RevertError, RevertOutcome};
pub use aidoc_validator as validator;
pub use aidoc_package as package;
pub use aidoc_package::{
    create_package, open_package, save_package, Manifest, Package, PackageError,
};
pub use aidoc_renderer as renderer;
pub use aidoc_exporter as exporter;
pub use aidoc_exporter::{export_html, export_markdown};