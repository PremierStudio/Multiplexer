//! In-memory hub, piped capture, and an embedded PTY session.
//!
//! [`EmbeddedSession`] hosts a system shell or `grok` inside Multiplexer
//! (ConPTY on Windows, posix_openpt on Unix). [`ProcessCapture`] stays for
//! piped `/C` commands. [`TerminalHub`] is still in-memory.

mod capture;
mod cmdline;
mod error;
mod hub;
mod id;
mod session;
mod spec;

pub use capture::ProcessCapture;
pub use error::TerminalError;
pub use hub::{TerminalHub, TerminalSnapshot, TerminalWatch};
pub use id::TerminalId;
pub use session::{ConptySession, EmbeddedSession};
pub use spec::TerminalSpec;

/// Drop CSI / OSC so an in-app pane is readable without a full VT grid.
pub fn visible_pty_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for n in chars.by_ref() {
                        if n.is_ascii_alphabetic() || n == '~' {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    for n in chars.by_ref() {
                        if n == '\u{7}' || n == '\u{1b}' {
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
            continue;
        }
        if c == '\r' {
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_pty_text_strips_csi() {
        assert_eq!(visible_pty_text("\u{1b}[32mhello\u{1b}[0m\r\n"), "hello\n");
        assert_eq!(visible_pty_text("plain"), "plain");
        assert_eq!(visible_pty_text(""), "");
        assert_eq!(visible_pty_text("\r\r"), "");
        assert_eq!(visible_pty_text("\u{1b}]0;title\u{7}ok"), "ok");
        assert_eq!(visible_pty_text("a\u{1b}[1mb"), "ab");
        assert_ne!(visible_pty_text("\u{1b}[1mX"), "\u{1b}[1mX");
        assert_ne!(visible_pty_text("hi\r\n"), "hi\r\n");
    }
}
