//! Terminal identity (`term-1`, `term-2`, ...).

use std::fmt;

/// Globally unique terminal identifier assigned by [`crate::TerminalHub::create`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalId(pub String);

impl TerminalId {
    /// Borrow the raw id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TerminalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for TerminalId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for TerminalId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_from() {
        let id = TerminalId::from("term-1");
        assert_eq!(id.as_str(), "term-1");
        assert_eq!(id.to_string(), "term-1");
        assert_eq!(id.0, "term-1");
        assert_eq!(TerminalId::from(String::from("term-2")).0, "term-2");
        assert_eq!(format!("{id}"), "term-1");
        assert_ne!(TerminalId::from("term-1"), TerminalId::from("term-2"));
    }
}
