//! JSON-RPC request router and WebSocket listen loop.
//!
//! The router is generic over [`SessionBackend`]. [`FakeBackend`] is the
//! in-crate double; [`ProviderBridge`] wraps a [`multiplexer_provider::ProviderAdapter`].
//! [`RuntimeBackend`] starts provider + resman + checkpoints together.

mod backend;
mod git;
mod listen;
mod provider_bridge;
mod runtime;
mod server;

pub use backend::{
    BackendError, FakeBackend, SessionBackend, SessionSnapshot, SessionStartParams, SessionSummary,
    StartedSession,
};
pub use git::{GitCatalog, WorktreeInfo};
pub use listen::{serve, serve_listener, ListenError};
pub use provider_bridge::ProviderBridge;
pub use runtime::RuntimeBackend;
pub use server::Server;
