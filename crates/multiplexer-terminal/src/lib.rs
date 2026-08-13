//! In-memory hub, piped capture, and an embedded PTY session.
//!
//! [`EmbeddedSession`] hosts a system shell or `grok` inside Multiplexer
//! (ConPTY on Windows, posix_openpt on Unix). [`ProcessCapture`] stays for
//! piped `/C` commands. [`TerminalHub`] is still in-memory.

mod capture;
mod cmdline;
mod error;
mod frame;
mod hub;
mod id;
mod keys;
mod session;
mod spec;

pub use capture::ProcessCapture;
pub use error::TerminalError;
pub use frame::{render_pty_chunk, PtyFrame, ONESHOT_COLS, ONESHOT_ROWS};
pub use hub::{TerminalHub, TerminalSnapshot, TerminalWatch};
pub use id::TerminalId;
pub use keys::{pty_grid_from_px, pty_input, pty_key_bytes, pty_paste_bytes, validate_pty_size};
pub use session::{ConptySession, EmbeddedSession};
pub use spec::TerminalSpec;

/// Last-frame render of a PTY chunk. Prefer a stateful [`PtyFrame`]
/// across reads so split CSI cannot leak and redraws replace the screen.
pub fn visible_pty_text(raw: &str) -> String {
    render_pty_chunk(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_pty_text_is_last_frame() {
        assert_eq!(visible_pty_text("\u{1b}[32mhello\u{1b}[0m\r\n"), "hello");
        assert_eq!(visible_pty_text("plain"), "plain");
        assert_eq!(visible_pty_text(""), "");
        assert_eq!(visible_pty_text("\r\r"), "");
        assert_eq!(visible_pty_text("\u{1b}]0;title\u{7}ok"), "ok");
        assert_eq!(visible_pty_text("a\u{1b}[1mb"), "ab");
        assert_eq!(visible_pty_text("hello\rX"), "Xello");
        assert_eq!(visible_pty_text("old\u{1b}[2J\u{1b}[Hnew"), "new");
        assert_eq!(visible_pty_text("old\u{1b}[2Jnew"), "   new");
        assert_ne!(visible_pty_text("\u{1b}[1mX"), "\u{1b}[1mX");
        assert_ne!(visible_pty_text("hi\r\n"), "hi\r\n");
        assert_ne!(visible_pty_text("hello\rX"), "helloX");
        assert_ne!(visible_pty_text("old\u{1b}[2J\u{1b}[Hnew"), "oldnew");
    }
}
