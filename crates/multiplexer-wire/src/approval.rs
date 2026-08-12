//! Approval-decision model (D12).
//!
//! The 4-way decision enum carried verbatim across the wire contract (plan/04),
//! the ProviderAdapter trait (plan/05), the orchestration command model
//! (plan/06), and security (plan/17). `AllowOnce` / `AllowAlways` are real
//! product features (permission modes), so this is an enum, never a boolean.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The canonical approval decision. Exactly four variants, serialized to the
/// wire spellings `allow` / `deny` / `allow_once` / `allow_always`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    /// Permit this single action; do not remember anything.
    Allow,
    /// Reject this action.
    Deny,
    /// Permit only this one occurrence; the next request asks again.
    AllowOnce,
    /// Permit and remember for the tool/scope so future requests auto-approve.
    AllowAlways,
}

impl ApprovalDecision {
    /// Does this decision permit the requested action?
    pub fn permits(self) -> bool {
        !matches!(self, Self::Deny)
    }

    /// Does this decision persist a grant for future requests?
    pub fn remembers(self) -> bool {
        matches!(self, Self::AllowAlways)
    }

    /// Parse a wire spelling into a decision.
    pub fn parse(s: &str) -> Result<Self, ApprovalDecisionParseError> {
        match s {
            "allow" => Ok(Self::Allow),
            "deny" => Ok(Self::Deny),
            "allow_once" => Ok(Self::AllowOnce),
            "allow_always" => Ok(Self::AllowAlways),
            other => Err(ApprovalDecisionParseError::UnknownVariant(other.to_owned())),
        }
    }
}

impl FromStr for ApprovalDecision {
    type Err = ApprovalDecisionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::AllowOnce => "allow_once",
            Self::AllowAlways => "allow_always",
        };
        f.write_str(s)
    }
}

/// Error produced when a wire string is not a valid decision.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApprovalDecisionParseError {
    #[error(
        "unknown approval decision '{0}': expected one of allow, deny, allow_once, allow_always"
    )]
    UnknownVariant(String),
}
