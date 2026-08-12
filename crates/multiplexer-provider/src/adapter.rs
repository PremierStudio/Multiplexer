//! [`ProviderAdapter`] command surface.

use std::path::PathBuf;

use multiplexer_wire::approval::ApprovalDecision;

use crate::error::ProviderError;
use crate::event::ProviderEvent;
use crate::ids::{ModelId, ProviderKind, SessionId};

/// Unified session-start params (D19). `resume` is accepted and ignored by
/// [`crate::FakeProvider`].
#[derive(Debug, Clone)]
pub struct SessionStartParams {
    /// Which backend should own the session.
    pub provider: ProviderKind,
    /// Model config to run.
    pub model: ModelId,
    /// Workspace root the session may touch.
    pub workspace: PathBuf,
    /// Optional first user turn, executed as part of start.
    pub initial_prompt: Option<String>,
    /// Backend resume cursor. Unused by the fake.
    pub resume: Option<String>,
}

/// A user turn to send into an existing session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnInput {
    /// Prompt text.
    pub text: String,
}

/// Point-in-time view of a live session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Session id.
    pub id: SessionId,
    /// Model selected at start.
    pub model: ModelId,
    /// Workspace selected at start.
    pub workspace: PathBuf,
    /// True while a turn has been accepted and not yet finished or interrupted.
    pub running: bool,
}

/// Backend-agnostic session commands. Outcomes arrive via [`ProviderEvent`].
pub trait ProviderAdapter: Send + Sync {
    /// Adapter identity.
    fn kind(&self) -> ProviderKind;

    /// Create a session and enqueue [`ProviderEvent::SessionReady`].
    fn start_session(&self, params: SessionStartParams) -> Result<SessionId, ProviderError>;

    /// Accept a user turn. Completion is [`ProviderEvent::TurnFinished`].
    fn send_turn(&self, session: &SessionId, turn: TurnInput) -> Result<(), ProviderError>;

    /// Stop the in-flight turn. No running turn is [`ProviderError::InvalidState`].
    fn interrupt_turn(&self, session: &SessionId) -> Result<(), ProviderError>;

    /// Answer a pending [`ProviderEvent::ApprovalRequested`].
    fn approval_respond(
        &self,
        session: &SessionId,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), ProviderError>;

    /// Drop the session. Further commands return [`ProviderError::NotFound`].
    fn session_stop(&self, session: &SessionId) -> Result<(), ProviderError>;

    /// Pop the next event for `session`, FIFO. Missing session or empty queue is `None`.
    fn poll_event(&self, session: &SessionId) -> Option<ProviderEvent>;

    /// Live session ids in start order.
    fn list_sessions(&self) -> Vec<SessionId>;

    /// Snapshot a live session.
    fn get_session(&self, session: &SessionId) -> Option<SessionSnapshot>;
}
