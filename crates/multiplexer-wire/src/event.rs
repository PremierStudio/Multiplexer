//! Server-pushed event notifications (plan/04 §3.5, §5).
//!
//! All server-pushed events use the single `event` method with a
//! discriminated `event` field. Events carry a `stream` (for pane routing),
//! a per-stream monotonic `seq` (gap detection / ordered replay), and an
//! optional `in_response_to` request id for correlation.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jsonrpc::{Id, Notification};
use crate::methods;

/// The canonical wire event kinds (plan/04 §5). Serialized to snake_case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    /// Incremental assistant text.
    AgentMessageChunk,
    /// Reasoning/thought stream.
    AgentThoughtChunk,
    /// Tool invocation started.
    ToolCall,
    /// Tool progress/completion.
    ToolCallUpdate,
    /// Structured plan object.
    Plan,
    /// Needs approval.
    PermissionRequest,
    /// Needs user input.
    UserInputRequest,
    /// New checkpoint created.
    Checkpoint,
    /// PTY bytes (base64).
    TerminalOutput,
    /// Terminal exited.
    TerminalExit,
    /// Network entry captured.
    HarEvent,
    /// Fan-out dashboard status.
    SubagentStatus,
    /// File watcher events.
    FsChange,
    /// Turn status.
    TurnStatus,
    /// Session status.
    SessionStatus,
    /// Periodic resource sample.
    TelemetryResources,
    /// Async error on a stream.
    Error,
}

/// A server-pushed event on a named stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamEvent {
    /// The stream this event belongs to, e.g. `turn:thr_01`.
    pub stream: String,
    /// The discriminated event kind.
    pub event: EventKind,
    /// Monotonically increasing per stream.
    pub seq: u64,
    /// The event payload.
    pub data: Value,
    /// The request id this event is a direct consequence of, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_response_to: Option<Id>,
}

impl StreamEvent {
    /// Build an event with no originating request.
    pub fn new(stream: String, event: EventKind, seq: u64, data: Value) -> Self {
        Self {
            stream,
            event,
            seq,
            data,
            in_response_to: None,
        }
    }

    /// Convert into the wire notification shape (method `event`).
    pub fn to_notification(&self) -> Notification {
        Notification::new(
            methods::EVENT,
            serde_json::to_value(self).expect("event serializes"),
        )
    }
}
