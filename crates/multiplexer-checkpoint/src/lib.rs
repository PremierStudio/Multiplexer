//! In-memory hidden-ref checkpoint store (plan/07, Phase 1.8 stub).
//!
//! Ids stand in for `refs/multiplexer/...` until a real git backend lands.
//! This crate does not spawn git or touch the filesystem.

mod error;
mod id;
mod store;

pub use error::CheckpointError;
pub use id::CheckpointId;
pub use store::{Checkpoint, CheckpointStore};
