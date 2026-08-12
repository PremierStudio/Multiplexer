//! Pure MCP lifecycle state machine (plan/21).
//!
//! No process spawn lives here. "Spawn" is instant and fake so the supervisor
//! can be tested without a real child. The later process supervisor sits on
//! multiplexer-resman containment.

mod config;
mod supervisor;

pub use config::{config_hash, ConfigHash, ServerConfig, ServerId};
pub use supervisor::{
    backoff_ms_for, LifecycleState, ServerHandle, Supervisor, SupervisorError, BACKOFF_BASE_MS,
    BACKOFF_CAP_MS, MAX_CONSECUTIVE_FAILURES,
};
