//! Errors from [`crate::AuthStore`] operations.

/// Failure from an auth store operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// Value looked like a raw token rather than a reference.
    #[error("plaintext secrets are forbidden")]
    PlaintextForbidden,
    /// No entry exists for the given name.
    #[error("not found: {0}")]
    NotFound(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_distinct() {
        assert_eq!(
            AuthError::PlaintextForbidden.to_string(),
            "plaintext secrets are forbidden"
        );
        assert_eq!(
            AuthError::NotFound("grok".into()).to_string(),
            "not found: grok"
        );
        assert_ne!(
            AuthError::PlaintextForbidden,
            AuthError::NotFound("grok".into())
        );
        assert_ne!(
            AuthError::NotFound("a".into()),
            AuthError::NotFound("b".into())
        );
    }
}
