//! In-process JSON-RPC request router (no TCP/WebSocket yet).
//!
//! The router is generic over [`SessionBackend`]. [`FakeBackend`] is the
//! in-crate double; [`ProviderBridge`] wraps a [`multiplexer_provider::ProviderAdapter`].

mod backend;
mod git;
mod provider_bridge;
mod server;

pub use backend::{
    BackendError, FakeBackend, SessionBackend, SessionSnapshot, SessionStartParams, SessionSummary,
    StartedSession,
};
pub use git::{GitCatalog, WorktreeInfo};
pub use provider_bridge::ProviderBridge;
pub use server::Server;
