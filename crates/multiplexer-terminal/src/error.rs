//! Errors from [`crate::TerminalHub`] operations.

use crate::TerminalId;

/// Failure from a terminal hub operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TerminalError {
    /// No live terminal exists for the given id.
    #[error("not found: {0}")]
    NotFound(TerminalId),
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
}
