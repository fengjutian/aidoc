//! AIDoc logical model.
//!
//! Maps to the AIDoc v0.1 spec §3 / §5-§30. This crate only owns types and
//! pure helpers — no I/O, no SQLite, no Zip. Higher crates compose these
//! types into a working runtime.

pub mod document;
pub mod id;
pub mod node;
pub mod operation;
pub mod provenance;
pub mod relation;
pub mod revision;

pub use document::Document;
pub use id::{AIDocError, NodeId, OpId, RevisionId, Result};
pub use node::{Node, NodeKind};
pub use operation::{Operation, OperationType, Patch};
pub use provenance::Provenance;
pub use relation::{Relation, RelationKind};
pub use revision::{Change, ChangeType, HashRef, Revision};