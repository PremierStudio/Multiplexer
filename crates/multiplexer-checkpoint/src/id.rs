//! Checkpoint identity (`cp-1`, `cp-2`, ...).

use std::fmt;

/// Globally unique checkpoint identifier assigned by [`crate::CheckpointStore::create`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    /// Borrow the raw id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for CheckpointId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CheckpointId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_from() {
        let id = CheckpointId::from("cp-1");
        assert_eq!(id.as_str(), "cp-1");
        assert_eq!(id.to_string(), "cp-1");
        assert_eq!(id.0, "cp-1");
        assert_eq!(CheckpointId::from(String::from("cp-2")).0, "cp-2");
        assert_eq!(format!("{id}"), "cp-1");
        assert_ne!(CheckpointId::from("cp-1"), CheckpointId::from("cp-2"));
    }
}
