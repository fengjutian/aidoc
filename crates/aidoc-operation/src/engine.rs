//! Convenience facade / error types re-exported from the operation engine.

pub use crate::apply::{ApplyError, ApplyOutcome};
pub use crate::check::CheckError;

/// All errors that can occur while running an Operation.
#[derive(Debug, thiserror::Error)]
pub enum OperationError {
    #[error("check failed: {0}")]
    Check(#[from] CheckError),

    #[error("apply failed: {0}")]
    Apply(#[from] ApplyError),
}

pub type OperationResult<T> = Result<T, OperationError>;