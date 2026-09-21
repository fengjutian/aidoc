//! Operation engine (spec §20-§22, §25-§27, §31).
//!
//! An [`Operation`] is a *declarative intent*. To execute it we
//!   1. validate the intent (id format, target exists, ...)
//!   2. check optimistic concurrency against `expected_revision`
//!   3. apply the patch under a SQLite transaction
//!   4. emit a `Change` and advance `is_head` to a new `Revision`
//!
//! Revert is handled by [`aidoc_history`]; here we only implement the
//! forward-looking ops.

pub mod apply;
pub mod branching;
pub mod check;
pub mod conflict;
pub mod engine;
pub mod handler;

pub use apply::{ApplyError, ApplyOutcome, apply_operation, apply_with_registry};
pub use branching::{BranchError, branch_head, checkout_branch, merge_branch};
pub use check::{CheckError, check_all, check_conflict, check_revision};
pub use conflict::{Conflict, ConflictKind};
pub use engine::{OperationError, OperationResult};
pub use handler::{ApplyContext, HandlerRegistry, OperationHandler};
