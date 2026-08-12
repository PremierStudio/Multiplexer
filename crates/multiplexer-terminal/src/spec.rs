//! Size and working directory for a terminal session.

use std::path::PathBuf;

/// How to create a terminal. PTY fields land later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSpec {
    pub cols: u16,
    pub rows: u16,
    pub cwd: PathBuf,
}

impl TerminalSpec {
    pub fn new(cols: u16, rows: u16, cwd: impl Into<PathBuf>) -> Self {
        Self {
            cols,
            rows,
            cwd: cwd.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stores_fields() {
        let spec = TerminalSpec::new(80, 24, "/tmp");
        assert_eq!(spec.cols, 80);
        assert_eq!(spec.rows, 24);
        assert_eq!(spec.cwd, PathBuf::from("/tmp"));
        assert_ne!(spec, TerminalSpec::new(81, 24, "/tmp"));
        assert_ne!(spec, TerminalSpec::new(80, 25, "/tmp"));
        assert_ne!(spec, TerminalSpec::new(80, 24, "/other"));
    }
}
