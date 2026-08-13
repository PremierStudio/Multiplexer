//! Hidden-git checkpoint store: RAM catalog plus `refs/multiplexer/checkpoints/*`.

mod error;
mod hidden;
mod id;
mod store;

pub use error::CheckpointError;
pub use hidden::{
    ref_for, CheckpointDiff, FakeGitExec, GitExec, GitOut, HiddenGitStore, ProcessGitExec,
    RevertOutcome,
};
pub use id::CheckpointId;
pub use store::{Checkpoint, CheckpointStore};
