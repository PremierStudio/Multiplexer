//! Git worktree porcelain parsing and a `GitRunner`-backed worktree service.
//!
//! Parsing is pure. Spawning git is injected via [`GitRunner`] (`FakeGit` for tests).

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod git;
pub mod porcelain;

pub use git::{FakeGit, GitCall, GitRunner, WorktreeError, WorktreeService};
pub use porcelain::{find_by_branch, parse_porcelain, PorcelainError, Worktree};
