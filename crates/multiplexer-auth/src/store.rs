//! Named store of [`crate::SecretRef`] values only.

use std::collections::HashMap;

use crate::{AuthError, SecretRef};

/// Put/get/delete by name. Values are references, never raw tokens.
pub trait AuthStore {
    fn put(&mut self, name: &str, value: SecretRef) -> Result<(), AuthError>;
    fn get(&self, name: &str) -> Option<SecretRef>;
    fn delete(&mut self, name: &str) -> Result<(), AuthError>;
}

/// In-memory [`AuthStore`] for tests and the local stub.
#[derive(Debug, Default)]
pub struct MemoryAuthStore {
    entries: HashMap<String, SecretRef>,
}

impl MemoryAuthStore {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Number of stored references.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl AuthStore for MemoryAuthStore {
    fn put(&mut self, name: &str, value: SecretRef) -> Result<(), AuthError> {
        if SecretRef::looks_like_plaintext(value.as_str()) {
            return Err(AuthError::PlaintextForbidden);
        }
        self.entries.insert(name.to_owned(), value);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<SecretRef> {
        self.entries.get(name).cloned()
    }

    fn delete(&mut self, name: &str) -> Result<(), AuthError> {
        self.entries
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| AuthError::NotFound(name.to_owned()))
    }
}
