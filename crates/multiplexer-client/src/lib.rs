//! Non-blocking grok turn jobs for the desktop.
//!
//! The GPUI frame loop must never call `grok -p`. [`spawn_grok_turn`] starts a
//! named worker thread and returns a channel the UI polls with [`try_recv`].
//! [`TurnRequest::program`] is the overridable binary (tests use a fake path).

mod command;
mod files;
mod tui;
mod turn;

pub use command::{
    spawn_command, windows_cmd, CommandRequest, CommandResult, SHELL_WORKER_THREAD_NAME,
};
pub use files::{list_project_files, list_project_tree, FileEntry, ListOptions};
pub use tui::{powershell_app_activate, spawn_grok_tui, TuiLaunch};
pub use turn::{spawn_grok_turn, try_recv, TurnError, TurnRequest, TurnResult, WORKER_THREAD_NAME};
