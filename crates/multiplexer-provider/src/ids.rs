//! Session, model, and provider identity types.

use std::fmt;

/// Provider-owned session identifier (`sess-1`, `sess-2`, ... for [`crate::FakeProvider`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

impl SessionId {
    /// Borrow the raw id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SessionId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Registry/model identifier selected at session start.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(pub String);

impl ModelId {
    /// Borrow the raw model string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ModelId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ModelId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Which backend implements [`crate::ProviderAdapter`].
///
/// OpenRouter/DeepSeek is a config variant of [`ProviderKind::GrokInProcess`], not a
/// distinct kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    /// Embedded grok-build runtime (not wired in this crate yet).
    GrokInProcess,
    /// Agent Client Protocol backend.
    Acp,
    /// In-memory test double.
    Fake,
}

impl ProviderKind {
    /// Every known kind, in declaration order.
    pub const ALL: [ProviderKind; 3] = [Self::GrokInProcess, Self::Acp, Self::Fake];

    /// Stable snake_case spelling used in diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GrokInProcess => "grok_in_process",
            Self::Acp => "acp",
            Self::Fake => "fake",
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_id_display_and_from() {
        let id = SessionId::from("sess-1");
        assert_eq!(id.as_str(), "sess-1");
        assert_eq!(id.to_string(), "sess-1");
        assert_eq!(SessionId::from(String::from("sess-2")).0, "sess-2");
    }

    #[test]
    fn model_id_display_and_from() {
        let id = ModelId::from("grok");
        assert_eq!(id.as_str(), "grok");
        assert_eq!(id.to_string(), "grok");
        assert_eq!(ModelId::from(String::from("other")).0, "other");
    }

    #[test]
    fn provider_kind_spellings() {
        assert_eq!(ProviderKind::GrokInProcess.as_str(), "grok_in_process");
        assert_eq!(ProviderKind::Acp.as_str(), "acp");
        assert_eq!(ProviderKind::Fake.as_str(), "fake");
        assert_eq!(ProviderKind::GrokInProcess.to_string(), "grok_in_process");
        assert_eq!(ProviderKind::Acp.to_string(), "acp");
        assert_eq!(ProviderKind::Fake.to_string(), "fake");
        assert_eq!(ProviderKind::ALL.len(), 3);
        assert_ne!(ProviderKind::GrokInProcess, ProviderKind::Acp);
        assert_ne!(ProviderKind::Acp, ProviderKind::Fake);
        assert_ne!(ProviderKind::GrokInProcess, ProviderKind::Fake);
    }
}
