//! AIDoc facade crate.
//!
//! Re-exports the public surface of every core crate so callers can simply
//! `use aidoc::*` and skip per-crate paths.

pub use aidoc_exporter as exporter;
pub use aidoc_exporter::{ExportFormat, ExportInput, Exporter, export_html, export_markdown};
pub use aidoc_history as history;
pub use aidoc_history::{RevertError, RevertOutcome, revert_to};
pub use aidoc_model as model;
pub use aidoc_model::*;
pub use aidoc_operation as operation;
pub use aidoc_operation::{
    ApplyContext, ApplyError, ApplyOutcome, Conflict, ConflictKind, HandlerRegistry,
    OperationError, OperationHandler, PreviewReport, apply_operation, apply_with_registry,
    preview_operation, preview_with_registry,
};
pub use aidoc_operation::preview::{NodeDiff, RelationDiff};
pub use aidoc_operation::{BranchError, branch_head, checkout_branch, merge_branch};
pub use aidoc_package as package;
pub use aidoc_package::{
    ImportError, Manifest, Package, PackageError, create_package, import_canonical_document,
    import_canonical_document_value, inline_image_assets, open_package, save_package,
};
pub use aidoc_renderer as renderer;
pub use aidoc_storage as storage;
pub use aidoc_storage::{AnyhowErr, SnapshotStrategy, Store, StoreError, crud};
pub use aidoc_validator as validator;
pub use aidoc_validator::{Finding, ValidationCategory, Validator};
