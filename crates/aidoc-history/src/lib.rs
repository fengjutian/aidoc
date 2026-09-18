//! History: revert = new revision (spec §27).
//!
//! Reverting must never delete history. To revert to `R102` we
//!   1. read the document state at R102
//!   2. overwrite the live node table with that state
//!   3. emit a `Revert` change for each affected node
//!   4. advance the head pointer to a brand-new `Revision`
//!
//! This means `list_revisions` keeps growing; nothing is destroyed.

pub mod revert;

pub use revert::{revert_to, RevertError, RevertOutcome};