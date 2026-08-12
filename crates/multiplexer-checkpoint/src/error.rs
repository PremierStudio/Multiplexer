//! Errors from [`crate::CheckpointStore`] operations.

use crate::CheckpointId;

/// Failure from a checkpoint store operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CheckpointError {
    /// No checkpoint exists for the given id.
    #[error("not found: {0}")]
    NotFound(CheckpointId),
}
