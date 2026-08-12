//! JSON-RPC 2.0 envelope types (plan/04 §3).
//!
//! The four message kinds from the spec: request, response, error response,
//! and notification. All carry the `jsonrpc: "2.0"` version field. The
//! `Message` enum is the codec's unit of encode/decode. Notifications and
//! server-pushed events have no `id`; requests and responses always do.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::RpcError;

/// The JSON-RPC version string required on every frame.
pub const JSONRPC_VERSION: &str = "2.0";

/// A request or response correlation id: an opaque string or an integer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    /// Opaque string id (recommended by plan/04 §3.1).
    String(String),
    /// Integer id.
    Number(i64),
}

/// A client-to-server request (plan/04 §3.1). Always carries an `id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The JSON-RPC version, always `"2.0"`.
    pub jsonrpc: String,
    /// Correlation id, echoed by the response.
    pub id: Id,
    /// The method name, e.g. `turn.send`.
    pub method: String,
    /// Named params object (never positional arrays).
    pub params: Value,
}

impl Request {
    /// Build a request with the canonical version field.
    pub fn new(id: Id, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            method: method.into(),
            params,
        }
    }
}

/// A server-to-client success response (plan/04 §3.2). Echoes the request id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The JSON-RPC version, always `"2.0"`.
    pub jsonrpc: String,
    /// The id of the request this responds to.
    pub id: Id,
    /// The success payload.
    pub result: Value,
}

impl Response {
    /// Build a response with the canonical version field.
    pub fn new(id: Id, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            result,
        }
    }
}

/// A server-to-client error response (plan/04 §3.3, §7). Echoes the request id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// The JSON-RPC version, always `"2.0"`.
    pub jsonrpc: String,
    /// The id of the request this responds to.
    pub id: Id,
    /// The structured error object.
    pub error: RpcError,
}

impl ErrorResponse {
    /// Build an error response with the canonical version field.
    pub fn new(id: Id, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id,
            error,
        }
    }
}

/// A fire-and-forget message with no `id` and therefore no response
/// (plan/04 §3.4). Also the carrier for server-pushed events (§3.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    /// The JSON-RPC version, always `"2.0"`.
    pub jsonrpc: String,
    /// The method name, e.g. `terminal.input` or `event`.
    pub method: String,
    /// Named params object.
    pub params: Value,
}

impl Notification {
    /// Build a notification with the canonical version field.
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: method.into(),
            params,
        }
    }
}

/// Any single frame on the wire, discriminated by shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// A request (has `id` and `method`).
    Request(Request),
    /// A success response (has `id` and `result`).
    Response(Response),
    /// An error response (has `id` and `error`).
    Error(ErrorResponse),
    /// A notification (has `method`, no `id`).
    Notification(Notification),
}

impl Message {
    /// The correlation id, if this message carries one.
    pub fn id(&self) -> Option<&Id> {
        match self {
            Message::Request(r) => Some(&r.id),
            Message::Response(r) => Some(&r.id),
            Message::Error(r) => Some(&r.id),
            Message::Notification(_) => None,
        }
    }
}
