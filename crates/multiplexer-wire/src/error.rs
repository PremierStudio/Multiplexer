//! Structured, machine-readable error model (plan/04 §7).
//!
//! Every error has a JSON-RPC integer `code` and, for application errors, a
//! stable `data.kind` string plus optional `data.details`. Clients switch on
//! `data.kind`, never on `message` text.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC standard error codes (plan/04 §7.1).
pub mod standard {
    /// Parse error.
    pub const PARSE_ERROR: i64 = -32700;
    /// Invalid request.
    pub const INVALID_REQUEST: i64 = -32600;
    /// Method not found.
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Invalid params (schema violation).
    pub const INVALID_PARAMS: i64 = -32602;
    /// Internal error.
    pub const INTERNAL_ERROR: i64 = -32603;
}

/// Multiplexer application error kinds (plan/04 §7.2). Each maps to a stable
/// JSON-RPC code and a snake_case `data.kind` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorKind {
    /// No/invalid session.
    AuthRequired,
    /// Session expired, re-handshake.
    AuthExpired,
    /// Bad/expired/replayed ticket.
    TicketInvalid,
    /// Scope violation.
    PermissionDenied,
    /// Resource (thread/pty/checkpoint) missing.
    NotFound,
    /// State conflict (e.g. turn already running).
    Conflict,
    /// Operation invalid in current state.
    InvalidState,
    /// Path traversal / outside worktree.
    PathInvalid,
    /// Upstream provider/adapter failure.
    ProviderError,
    /// Backpressure / quota exceeded.
    RateLimited,
    /// Capability not negotiated.
    Unsupported,
    /// Subscribed stream no longer exists.
    StreamClosed,
    /// Client/server version drift.
    ProtocolVersionMismatch,
}

impl AppErrorKind {
    /// The JSON-RPC code for this application error kind.
    pub fn code(self) -> i64 {
        match self {
            Self::AuthRequired => -32000,
            Self::AuthExpired => -32001,
            Self::TicketInvalid => -32002,
            Self::PermissionDenied => -32003,
            Self::NotFound => -32004,
            Self::Conflict => -32005,
            Self::InvalidState => -32006,
            Self::PathInvalid => -32007,
            Self::ProviderError => -32008,
            Self::RateLimited => -32009,
            Self::Unsupported => -32010,
            Self::StreamClosed => -32011,
            Self::ProtocolVersionMismatch => -32012,
        }
    }
}

/// The structured error object carried in an error response (plan/04 §7.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    /// JSON-RPC integer code.
    pub code: i64,
    /// Human-readable message (localized client-side).
    pub message: String,
    /// Optional structured data: `kind` (stable identifier) and `details`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ErrorData>,
}

/// The `data` field of an error: a stable `kind` plus optional `details`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorData {
    /// Stable machine-readable error identifier.
    pub kind: String,
    /// Optional structured details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl RpcError {
    /// Build a standard JSON-RPC error with no `data`.
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Build an application error from a kind, deriving code and `data.kind`.
    pub fn app(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self::app_with_details(kind, message, None)
    }

    /// Build an application error with structured details.
    pub fn app_with_details(
        kind: AppErrorKind,
        message: impl Into<String>,
        details: Option<Value>,
    ) -> Self {
        Self {
            code: kind.code(),
            message: message.into(),
            data: Some(ErrorData {
                kind: serde_json::to_string(&kind)
                    .expect("AppErrorKind always serializes")
                    .trim_matches('"')
                    .to_owned(),
                details,
            }),
        }
    }
}
