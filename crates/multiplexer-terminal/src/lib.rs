//! In-memory terminal hub (plan/08 stub).
//!
//! PTY spawn is later. This crate owns ids, a cols/rows/cwd spec, and a hub
//! that records input in a buffer. Dropping the hub marks every session dead.

mod error;
mod hub;
mod id;
mod spec;

pub use error::TerminalError;
pub use hub::{TerminalHub, TerminalSnapshot, TerminalWatch};
pub use id::TerminalId;
pub use spec::TerminalSpec;
