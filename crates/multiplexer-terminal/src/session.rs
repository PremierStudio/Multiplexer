//! Interactive child attached to a PTY (ConPTY on Windows, posix on Unix).

#[cfg(not(any(windows, unix)))]
mod stub;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(not(any(windows, unix)))]
pub use stub::EmbeddedSession;
#[cfg(unix)]
pub use unix::EmbeddedSession;
#[cfg(windows)]
pub use windows::EmbeddedSession;

/// Alias used by plan/08 and the desktop TUI host.
pub type ConptySession = EmbeddedSession;
