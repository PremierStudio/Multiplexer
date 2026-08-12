//! Adapter errors, aligned with wire [`AppErrorKind`] names.

use multiplexer_wire::error::AppErrorKind;

/// Command rejection from a [`crate::ProviderAdapter`].
///
/// Variants map onto the wire kinds `not_found`, `conflict`, `invalid_state`,
/// and `provider_error`. Turn outcomes still arrive as [`crate::ProviderEvent`]s.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// Session, request, or other resource does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// State conflict, typically a turn already running.
    #[error("conflict: {0}")]
    Conflict(String),
    /// Operation is illegal in the current session state.
    #[error("invalid state: {0}")]
    InvalidState(String),
    /// Backend/adapter failure.
    #[error("provider error: {0}")]
    Provider(String),
}

impl ProviderError {
    /// Wire [`AppErrorKind`] this error should surface as.
    pub fn kind(&self) -> AppErrorKind {
        match self {
            Self::NotFound(_) => AppErrorKind::NotFound,
            Self::Conflict(_) => AppErrorKind::Conflict,
            Self::InvalidState(_) => AppErrorKind::InvalidState,
            Self::Provider(_) => AppErrorKind::ProviderError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_to_wire_kinds() {
        assert_eq!(
            ProviderError::NotFound("sess-1".into()).kind(),
            AppErrorKind::NotFound
        );
        assert_eq!(
            ProviderError::Conflict("turn already running".into()).kind(),
            AppErrorKind::Conflict
        );
        assert_eq!(
            ProviderError::InvalidState("no running turn".into()).kind(),
            AppErrorKind::InvalidState
        );
        assert_eq!(
            ProviderError::Provider("backend down".into()).kind(),
            AppErrorKind::ProviderError
        );
    }

    #[test]
    fn display_includes_detail() {
        assert_eq!(
            ProviderError::NotFound("session sess-9".into()).to_string(),
            "not found: session sess-9"
        );
        assert_eq!(
            ProviderError::Conflict("turn already running".into()).to_string(),
            "conflict: turn already running"
        );
        assert_eq!(
            ProviderError::InvalidState("no running turn".into()).to_string(),
            "invalid state: no running turn"
        );
        assert_eq!(
            ProviderError::Provider("timeout".into()).to_string(),
            "provider error: timeout"
        );
    }

    #[test]
    fn variants_are_distinct() {
        let not_found = ProviderError::NotFound("x".into());
        let conflict = ProviderError::Conflict("x".into());
        let invalid = ProviderError::InvalidState("x".into());
        let provider = ProviderError::Provider("x".into());
        assert_ne!(not_found, conflict);
        assert_ne!(not_found, invalid);
        assert_ne!(not_found, provider);
        assert_ne!(conflict, invalid);
        assert_ne!(conflict, provider);
        assert_ne!(invalid, provider);
        assert_ne!(not_found.kind(), conflict.kind());
        assert_ne!(invalid.kind(), provider.kind());
        assert_ne!(conflict.kind(), invalid.kind());
    }
}
