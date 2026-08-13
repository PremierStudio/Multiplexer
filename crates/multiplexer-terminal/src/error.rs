//! Errors from [`crate::TerminalHub`] and [`crate::ProcessCapture`].

use crate::TerminalId;

/// Failure from a terminal hub or process-capture operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalError {
    /// No live terminal exists for the given id.
    #[error("not found: {0}")]
    NotFound(TerminalId),
    /// `Command::spawn` failed.
    #[error("spawn `{program}`: {message}")]
    Spawn { program: String, message: String },
    /// Stdin write, kill, or other child I/O failed.
    #[error("{0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_id() {
        let err = TerminalError::NotFound(TerminalId::from("term-9"));
        assert_eq!(err.to_string(), "not found: term-9");
        assert_eq!(err, TerminalError::NotFound(TerminalId::from("term-9")));
        assert_ne!(err, TerminalError::NotFound(TerminalId::from("term-1")));
    }

    #[test]
    fn spawn_and_io_display() {
        let spawn = TerminalError::Spawn {
            program: "cmd.exe".into(),
            message: "not found".into(),
        };
        assert_eq!(spawn.to_string(), "spawn `cmd.exe`: not found");
        assert_eq!(
            TerminalError::Io("stdin closed".into()).to_string(),
            "stdin closed"
        );
        assert_ne!(spawn, TerminalError::Io("stdin closed".into()));
    }
}
