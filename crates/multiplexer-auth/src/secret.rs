//! Reference to a secret. The inner strings are names or `op://` paths, never tokens.

use std::fmt;

/// How to locate a secret. No variant stores a raw credential.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SecretRef {
    /// `op://Vault/Item/field` (resolved via the session cache, never live `op`).
    Op(String),
    /// `${ENV_VAR}` interpolation reference.
    Env(String),
    /// OS keychain account/service name.
    Keychain(String),
}

impl SecretRef {
    /// Classify `raw` as a reference, or reject a token-shaped value.
    pub fn parse(raw: &str) -> Result<Self, crate::AuthError> {
        if Self::looks_like_plaintext(raw) {
            return Err(crate::AuthError::PlaintextForbidden);
        }
        if raw.starts_with("op://") {
            Ok(Self::Op(raw.to_owned()))
        } else if raw.starts_with("${") {
            Ok(Self::Env(raw.to_owned()))
        } else {
            Ok(Self::Keychain(raw.to_owned()))
        }
    }

    /// True when `raw` is longer than 20 chars and has no `op://` or `${` prefix.
    pub fn looks_like_plaintext(raw: &str) -> bool {
        raw.len() > 20 && !raw.starts_with("op://") && !raw.starts_with("${")
    }

    /// Borrow the stored reference string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Op(s) | Self::Env(s) | Self::Keychain(s) => s,
        }
    }
}

impl fmt::Display for SecretRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthError;

    #[test]
    fn parse_routes_by_prefix() {
        let op = SecretRef::parse("op://Vault/Item/field").unwrap();
        let env = SecretRef::parse("${OPENROUTER_API_KEY}").unwrap();
        let key = SecretRef::parse("grok-key").unwrap();
        assert_eq!(op, SecretRef::Op("op://Vault/Item/field".into()));
        assert_eq!(env, SecretRef::Env("${OPENROUTER_API_KEY}".into()));
        assert_eq!(key, SecretRef::Keychain("grok-key".into()));
        assert_eq!(op.as_str(), "op://Vault/Item/field");
        assert_eq!(env.as_str(), "${OPENROUTER_API_KEY}");
        assert_eq!(key.as_str(), "grok-key");
        assert_eq!(op.to_string(), "op://Vault/Item/field");
        assert_eq!(env.to_string(), "${OPENROUTER_API_KEY}");
        assert_eq!(format!("{key}"), "grok-key");
        assert_ne!(op, env);
        assert_ne!(env, key);
        assert_ne!(op, key);
    }

    #[test]
    fn twenty_char_keychain_is_allowed() {
        let raw = "abcdefghijklmnopqrst";
        assert_eq!(raw.len(), 20);
        assert!(!SecretRef::looks_like_plaintext(raw));
        assert_eq!(
            SecretRef::parse(raw).unwrap(),
            SecretRef::Keychain(raw.into())
        );
    }

    #[test]
    fn twenty_one_char_without_prefix_is_forbidden() {
        let raw = "abcdefghijklmnopqrstu";
        assert_eq!(raw.len(), 21);
        assert!(SecretRef::looks_like_plaintext(raw));
        assert_eq!(
            SecretRef::parse(raw).unwrap_err(),
            AuthError::PlaintextForbidden
        );
    }

    #[test]
    fn long_op_and_env_refs_are_allowed() {
        let op = "op://Vault/VeryLongItemName/field";
        let env = "${VERY_LONG_ENVIRONMENT_VARIABLE}";
        assert!(op.len() > 20);
        assert!(env.len() > 20);
        assert!(!SecretRef::looks_like_plaintext(op));
        assert!(!SecretRef::looks_like_plaintext(env));
        assert!(matches!(SecretRef::parse(op).unwrap(), SecretRef::Op(_)));
        assert!(matches!(SecretRef::parse(env).unwrap(), SecretRef::Env(_)));
    }

    #[test]
    fn near_miss_prefixes_are_plaintext_when_long() {
        assert!(SecretRef::looks_like_plaintext("op:/not-a-valid-ref-xxx"));
        assert!(SecretRef::looks_like_plaintext("$NOT_AN_ENV_REF_XXXXX"));
        assert_eq!(
            SecretRef::parse("op:/not-a-valid-ref-xxx").unwrap_err(),
            AuthError::PlaintextForbidden
        );
        assert_eq!(
            SecretRef::parse("$NOT_AN_ENV_REF_XXXXX").unwrap_err(),
            AuthError::PlaintextForbidden
        );
    }

    #[test]
    fn empty_and_short_are_keychain() {
        assert_eq!(
            SecretRef::parse("").unwrap(),
            SecretRef::Keychain("".into())
        );
        assert_eq!(
            SecretRef::parse("x").unwrap(),
            SecretRef::Keychain("x".into())
        );
        assert!(!SecretRef::looks_like_plaintext(""));
        assert!(!SecretRef::looks_like_plaintext("op:/short"));
    }
}
