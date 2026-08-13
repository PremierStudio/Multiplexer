//! Git worktree porcelain parsing and a `GitRunner`-backed worktree service.
//!
//! Parsing is pure. Spawning git is injected via [`GitRunner`] (`FakeGit` for tests).

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod git;
pub mod porcelain;
pub mod reminder;

pub use git::{FakeGit, GitCall, GitRunner, ProcessGit, WorktreeError, WorktreeService};
pub use porcelain::{find_by_branch, parse_porcelain, PorcelainError, Worktree};
pub use reminder::{reminder_from_list, Reminder};
