//! Provider adapter seam (plan/05): one command trait, one event enum, a fake.
//!
//! Real grok-build embedding is out of scope for this crate revision.

mod adapter;
mod error;
mod event;
mod fake;
mod ids;

pub use adapter::{ProviderAdapter, SessionSnapshot, SessionStartParams, TurnInput};
pub use error::ProviderError;
pub use event::ProviderEvent;
pub use fake::FakeProvider;
pub use ids::{ModelId, ProviderKind, SessionId};
