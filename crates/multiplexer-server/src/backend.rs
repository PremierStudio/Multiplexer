//! Local session backend. Mirrors the provider adapter surface the router
//! needs so this crate is not blocked on `multiplexer-provider`.

use std::collections::{HashMap, HashSet};

use multiplexer_wire::approval::ApprovalDecision;
use multiplexer_wire::event::{EventKind, StreamEvent};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

/// Failures from a [`SessionBackend`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BackendError {
    /// No session with this id.
    #[error("session not found: {session_id}")]
    NotFound { session_id: String },
    /// Provider rejected the command.
    #[error("{message}")]
    Provider {
        kind: multiplexer_wire::error::AppErrorKind,
        message: String,
    },
}

/// Unified session-start params (D19), trimmed to the router fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionStartParams {
    pub provider: String,
    pub model: String,
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_prompt: Option<String>,
}

/// Result of [`SessionBackend::start`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartedSession {
    pub session_id: String,
}

/// Compact session row for `session.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub model: String,
    pub workspace: String,
}

/// Full session snapshot for `session.get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub workspace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_prompt: Option<String>,
}

impl SessionSnapshot {
    /// Compact list row (id, model, workspace only).
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            id: self.id.clone(),
            model: self.model.clone(),
            workspace: self.workspace.clone(),
        }
    }
}

/// Session store + event drain used by the request router.
pub trait SessionBackend {
    fn start(&mut self, params: SessionStartParams) -> Result<StartedSession, BackendError>;
    fn list(&self) -> Vec<SessionSummary>;
    fn get(&self, session_id: &str) -> Result<SessionSnapshot, BackendError>;
    fn stop(&mut self, session_id: &str) -> Result<(), BackendError>;
    fn send_turn(&mut self, session_id: &str, text: &str) -> Result<(), BackendError>;
    fn interrupt(&mut self, session_id: &str) -> Result<(), BackendError>;
    fn approval_respond(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError>;
    fn drain_events(&mut self) -> Vec<StreamEvent>;
}

/// In-memory backend for tests and the in-process router.
#[derive(Debug, Default)]
pub struct FakeBackend {
    next_id: u64,
    sessions: HashMap<String, SessionSnapshot>,
    order: Vec<String>,
    events: Vec<StreamEvent>,
    seq_by_stream: HashMap<String, u64>,
    pending_approvals: HashSet<(String, String)>,
}

impl FakeBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn alloc_id(&mut self) -> String {
        self.next_id += 1;
        format!("sess_{}", self.next_id)
    }

    fn require(&self, session_id: &str) -> Result<&SessionSnapshot, BackendError> {
        self.sessions
            .get(session_id)
            .ok_or_else(|| BackendError::NotFound {
                session_id: session_id.to_owned(),
            })
    }

    /// Track a pending approval so [`SessionBackend::approval_respond`] can succeed.
    pub fn request_approval(
        &mut self,
        session_id: &str,
        request_id: &str,
    ) -> Result<(), BackendError> {
        self.require(session_id)?;
        self.pending_approvals
            .insert((session_id.to_owned(), request_id.to_owned()));
        Ok(())
    }

    fn push_event(&mut self, stream: String, event: EventKind, data: Value) {
        let seq = {
            let slot = self.seq_by_stream.entry(stream.clone()).or_insert(0);
            *slot += 1;
            *slot
        };
        self.events.push(StreamEvent::new(stream, event, seq, data));
    }
}

impl SessionBackend for FakeBackend {
    fn start(&mut self, params: SessionStartParams) -> Result<StartedSession, BackendError> {
        let session_id = self.alloc_id();
        let snapshot = SessionSnapshot {
            id: session_id.clone(),
            provider: params.provider,
            model: params.model,
            workspace: params.workspace,
            initial_prompt: params.initial_prompt,
        };
        self.sessions.insert(session_id.clone(), snapshot);
        self.order.push(session_id.clone());
        self.push_event(
            format!("session:{session_id}"),
            EventKind::SessionStatus,
            json!({ "session_id": session_id, "status": "ready" }),
        );
        Ok(StartedSession { session_id })
    }

    fn list(&self) -> Vec<SessionSummary> {
        self.order
            .iter()
            .map(|id| {
                self.sessions
                    .get(id)
                    .expect("order and sessions stay in sync")
                    .summary()
            })
            .collect()
    }

    fn get(&self, session_id: &str) -> Result<SessionSnapshot, BackendError> {
        self.require(session_id).cloned()
    }

    fn stop(&mut self, session_id: &str) -> Result<(), BackendError> {
        self.require(session_id)?;
        self.sessions.remove(session_id);
        self.order.retain(|id| id != session_id);
        self.pending_approvals.retain(|(id, _)| id != session_id);
        Ok(())
    }

    fn send_turn(&mut self, session_id: &str, text: &str) -> Result<(), BackendError> {
        self.require(session_id)?;
        self.push_event(
            format!("turn:{session_id}"),
            EventKind::AgentMessageChunk,
            json!({ "text": text }),
        );
        Ok(())
    }

    fn interrupt(&mut self, session_id: &str) -> Result<(), BackendError> {
        self.require(session_id)?;
        Ok(())
    }

    fn approval_respond(
        &mut self,
        session_id: &str,
        request_id: &str,
        _decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        self.require(session_id)?;
        if !self
            .pending_approvals
            .remove(&(session_id.to_owned(), request_id.to_owned()))
        {
            return Err(BackendError::NotFound {
                session_id: format!("approval {request_id}"),
            });
        }
        Ok(())
    }

    fn drain_events(&mut self) -> Vec<StreamEvent> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(model: &str) -> SessionStartParams {
        SessionStartParams {
            provider: "grok".into(),
            model: model.into(),
            workspace: "/w".into(),
            initial_prompt: None,
        }
    }

    #[test]
    fn ids_are_sess_n_starting_at_one() {
        let mut backend = FakeBackend::new();
        assert_eq!(backend.start(params("m")).unwrap().session_id, "sess_1");
        assert_eq!(backend.start(params("n")).unwrap().session_id, "sess_2");
    }

    #[test]
    fn drain_clears_queued_events() {
        let mut backend = FakeBackend::new();
        backend.start(params("m")).unwrap();
        assert_eq!(backend.drain_events().len(), 1);
        assert!(backend.drain_events().is_empty());
    }

    #[test]
    fn summary_copies_list_fields_only() {
        let snap = SessionSnapshot {
            id: "s".into(),
            provider: "grok".into(),
            model: "m".into(),
            workspace: "/w".into(),
            initial_prompt: Some("p".into()),
        };
        let sum = snap.summary();
        assert_eq!(sum.id, "s");
        assert_eq!(sum.model, "m");
        assert_eq!(sum.workspace, "/w");
    }

    #[test]
    fn not_found_display_includes_id() {
        let err = BackendError::NotFound {
            session_id: "sess_x".into(),
        };
        assert_eq!(err.to_string(), "session not found: sess_x");
    }

    #[test]
    fn interrupt_unknown_is_not_found() {
        let mut backend = FakeBackend::new();
        let err = backend.interrupt("sess_missing").unwrap_err();
        assert_eq!(
            err,
            BackendError::NotFound {
                session_id: "sess_missing".into(),
            }
        );
    }

    #[test]
    fn interrupt_known_session_ok() {
        let mut backend = FakeBackend::new();
        let id = backend.start(params("m")).unwrap().session_id;
        backend.interrupt(&id).unwrap();
    }

    #[test]
    fn approval_unknown_session_is_not_found() {
        let mut backend = FakeBackend::new();
        let err = backend
            .approval_respond("sess_missing", "req-1", ApprovalDecision::Allow)
            .unwrap_err();
        assert_eq!(
            err,
            BackendError::NotFound {
                session_id: "sess_missing".into(),
            }
        );
    }

    #[test]
    fn approval_unknown_request_is_not_found() {
        let mut backend = FakeBackend::new();
        let id = backend.start(params("m")).unwrap().session_id;
        let err = backend
            .approval_respond(&id, "req-missing", ApprovalDecision::Deny)
            .unwrap_err();
        assert_eq!(
            err,
            BackendError::NotFound {
                session_id: "approval req-missing".into(),
            }
        );
    }

    #[test]
    fn approval_tracked_request_succeeds_once() {
        let mut backend = FakeBackend::new();
        let id = backend.start(params("m")).unwrap().session_id;
        backend.request_approval(&id, "req-1").unwrap();
        backend
            .approval_respond(&id, "req-1", ApprovalDecision::AllowAlways)
            .unwrap();
        let err = backend
            .approval_respond(&id, "req-1", ApprovalDecision::Allow)
            .unwrap_err();
        assert!(matches!(err, BackendError::NotFound { .. }));
    }

    #[test]
    fn request_approval_unknown_session_is_not_found() {
        let mut backend = FakeBackend::new();
        assert!(backend.request_approval("sess_x", "req-1").is_err());
    }

    #[test]
    fn stop_drops_pending_approvals() {
        let mut backend = FakeBackend::new();
        let id = backend.start(params("m")).unwrap().session_id;
        backend.request_approval(&id, "req-1").unwrap();
        backend.stop(&id).unwrap();
        assert!(backend
            .approval_respond(&id, "req-1", ApprovalDecision::Allow)
            .is_err());
    }
}
