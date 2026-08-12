//! Multiplexer core resource manager: CPU core bitmap allocation for sessions.

pub mod bitmap;

pub use bitmap::{CoreBitmap, ResmanError, SessionAlloc, SessionId};
