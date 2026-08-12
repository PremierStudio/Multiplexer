//! Auth store that holds [`SecretRef`] values only (plan/17).
//!
//! Raw tokens never live in these structs. A value that looks like plaintext
//! (length > 20 and no `op://` or `${` prefix) is rejected.

mod error;
mod secret;
mod store;

pub use error::AuthError;
pub use secret::SecretRef;
pub use store::{AuthStore, MemoryAuthStore};
