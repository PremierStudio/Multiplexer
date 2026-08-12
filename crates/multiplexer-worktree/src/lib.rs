//! Parsing of `git worktree list --porcelain` output.
//!
//! This crate is a pure parser: it turns porcelain-formatted text into typed
//! `Worktree` records. Spawning git lives elsewhere.

pub mod porcelain;

pub use porcelain::{find_by_branch, parse_porcelain, PorcelainError, Worktree};
