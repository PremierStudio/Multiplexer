//! Recover composer lines from bytes written into a hosted grok PTY.

/// Accumulates printable keystrokes until Enter.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TuiInputBuf {
    line: String,
    skipping_esc: bool,
}

impl TuiInputBuf {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw PTY input. Completed lines (Enter) are returned trimmed.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(bytes);
        let mut out = Vec::new();
        for ch in text.chars() {
            if self.skipping_esc {
                if ch.is_ascii_alphabetic() || ch == '~' {
                    self.skipping_esc = false;
                }
                continue;
            }
            match ch {
                '\u{1b}' => self.skipping_esc = true,
                '\r' | '\n' => {
                    let line = self.line.trim().to_owned();
                    self.line.clear();
                    if !line.is_empty() {
                        out.push(line);
                    }
                }
                '\u{8}' | '\u{7f}' => {
                    self.line.pop();
                }
                '\t' => self.line.push(' '),
                c if c.is_control() => {}
                c => self.line.push(c),
            }
        }
        out
    }

    pub fn pending(&self) -> &str {
        &self.line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_emits_trimmed_line() {
        let mut b = TuiInputBuf::new();
        assert!(b.feed(b"hello").is_empty());
        assert_eq!(b.pending(), "hello");
        assert_eq!(b.feed(b"\r"), vec!["hello".to_owned()]);
        assert!(b.pending().is_empty());
        assert!(b.feed(b"\r").is_empty());
        assert_ne!(b.feed(b"x\r"), vec!["hello".to_owned()]);
    }

    #[test]
    fn backspace_and_arrows_do_not_leak() {
        let mut b = TuiInputBuf::new();
        b.feed(b"ab");
        b.feed(&[0x7f]);
        b.feed(b"c");
        assert_eq!(b.feed(b"\n"), vec!["ac".to_owned()]);
        let mut arrows = TuiInputBuf::new();
        arrows.feed(b"ok\x1b[A\x1b[C!\r");
        assert_eq!(arrows.feed(b""), Vec::<String>::new());
        let mut a2 = TuiInputBuf::new();
        assert_eq!(a2.feed(b"ok\x1b[A!"), Vec::<String>::new());
        assert_eq!(a2.feed(b"\r"), vec!["ok!".to_owned()]);
        assert_ne!(a2.pending(), "ok\x1b[A!");
    }

    #[test]
    fn paste_crlf_is_one_line() {
        let mut b = TuiInputBuf::new();
        assert_eq!(
            b.feed(b"one\r\ntwo\r"),
            vec!["one".to_owned(), "two".to_owned()]
        );
        assert!(b.pending().is_empty());
    }
}
