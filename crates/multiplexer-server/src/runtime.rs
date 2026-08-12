//! [`SessionBackend`] over [`multiplexer_core::SessionRuntime`].

use std::path::PathBuf;

use multiplexer_core::{SessionRuntime, SessionRuntimeError};
use multiplexer_provider::{
    ModelId, ProviderAdapter, ProviderError, ProviderEvent, ProviderKind, SessionId,
    SessionStartParams as ProviderStart, TurnInput,
};
use multiplexer_wire::approval::ApprovalDecision;
use multiplexer_wire::error::AppErrorKind;
use multiplexer_wire::event::{EventKind, StreamEvent};
use serde_json::json;

use crate::backend::{
    BackendError, SessionBackend, SessionSnapshot, SessionStartParams, SessionSummary,
    StartedSession,
};

/// Router backend that starts provider + resman + checkpoint together.
pub struct RuntimeBackend {
    runtime: SessionRuntime,
    seq: u64,
}

impl Default for RuntimeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBackend {
    pub fn new() -> Self {
        Self {
            runtime: SessionRuntime::new(),
            seq: 0,
        }
    }

    pub fn runtime(&self) -> &SessionRuntime {
        &self.runtime
    }

    fn map_err(err: SessionRuntimeError) -> BackendError {
        match err {
            SessionRuntimeError::Provider(ProviderError::NotFound(id)) => {
                BackendError::NotFound { session_id: id }
            }
            SessionRuntimeError::Provider(other) => BackendError::Provider {
                kind: other.kind(),
                message: other.to_string(),
            },
            SessionRuntimeError::Resman(err) => BackendError::Provider {
                kind: AppErrorKind::ProviderError,
                message: err.to_string(),
            },
        }
    }

    fn parse_kind(raw: &str) -> ProviderKind {
        match raw {
            "acp" => ProviderKind::Acp,
            "grok" | "grok_in_process" => ProviderKind::GrokInProcess,
            _ => ProviderKind::Fake,
        }
    }

    fn push_event(&mut self, ev: ProviderEvent, out: &mut Vec<StreamEvent>) {
        self.seq += 1;
        let session = ev.session().as_str().to_owned();
        let (event, data) = match ev {
            ProviderEvent::SessionReady { session } => (
                EventKind::SessionStatus,
                json!({ "session_id": session.as_str(), "status": "ready" }),
            ),
            ProviderEvent::TextDelta { text, .. } => {
                (EventKind::AgentMessageChunk, json!({ "text": text }))
            }
            ProviderEvent::TurnFinished { .. } => {
                (EventKind::TurnStatus, json!({ "status": "finished" }))
            }
            ProviderEvent::TurnFailed { message, .. } => (
                EventKind::TurnStatus,
                json!({ "status": "failed", "message": message }),
            ),
            ProviderEvent::ApprovalRequested {
                request_id, tool, ..
            } => (
                EventKind::PermissionRequest,
                json!({ "request_id": request_id, "tool": tool }),
            ),
        };
        out.push(StreamEvent::new(
            format!("session:{session}"),
            event,
            self.seq,
            data,
        ));
    }
}

impl SessionBackend for RuntimeBackend {
    fn start(&mut self, params: SessionStartParams) -> Result<StartedSession, BackendError> {
        let id = self
            .runtime
            .start(ProviderStart {
                provider: Self::parse_kind(&params.provider),
                model: ModelId(params.model),
                workspace: PathBuf::from(params.workspace),
                initial_prompt: params.initial_prompt,
                resume: None,
            })
            .map_err(Self::map_err)?;
        Ok(StartedSession {
            session_id: id.to_string(),
        })
    }

    fn list(&self) -> Vec<SessionSummary> {
        self.runtime
            .provider()
            .list_sessions()
            .into_iter()
            .filter_map(|id| {
                let snap = self.runtime.provider().get_session(&id)?;
                Some(SessionSummary {
                    id: snap.id.to_string(),
                    model: snap.model.to_string(),
                    workspace: snap.workspace.to_string_lossy().into_owned(),
                })
            })
            .collect()
    }

    fn get(&self, session_id: &str) -> Result<SessionSnapshot, BackendError> {
        let id = SessionId::from(session_id);
        let snap =
            self.runtime
                .provider()
                .get_session(&id)
                .ok_or_else(|| BackendError::NotFound {
                    session_id: session_id.to_owned(),
                })?;
        Ok(SessionSnapshot {
            id: snap.id.to_string(),
            provider: self.runtime.provider().kind().as_str().to_owned(),
            model: snap.model.to_string(),
            workspace: snap.workspace.to_string_lossy().into_owned(),
            initial_prompt: None,
        })
    }

    fn stop(&mut self, session_id: &str) -> Result<(), BackendError> {
        self.runtime
            .stop(&SessionId::from(session_id))
            .map_err(Self::map_err)
    }

    fn send_turn(&mut self, session_id: &str, text: &str) -> Result<(), BackendError> {
        self.runtime
            .provider()
            .send_turn(
                &SessionId::from(session_id),
                TurnInput {
                    text: text.to_owned(),
                },
            )
            .map_err(|err| Self::map_err(SessionRuntimeError::from(err)))
    }

    fn interrupt(&mut self, session_id: &str) -> Result<(), BackendError> {
        self.runtime
            .provider()
            .interrupt_turn(&SessionId::from(session_id))
            .map_err(|err| Self::map_err(SessionRuntimeError::from(err)))
    }

    fn approval_respond(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        self.runtime
            .provider()
            .approval_respond(&SessionId::from(session_id), request_id, decision)
            .map_err(|err| Self::map_err(SessionRuntimeError::from(err)))
    }

    fn drain_events(&mut self) -> Vec<StreamEvent> {
        let ids = self.runtime.provider().list_sessions();
        let mut out = Vec::new();
        for id in ids {
            while let Some(ev) = self.runtime.provider().poll_event(&id) {
                self.push_event(ev, &mut out);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::SessionBackend;
    use multiplexer_core::SessionRuntimeError;
    use multiplexer_resman::ManagerError;
    use multiplexer_wire::approval::ApprovalDecision;
    use multiplexer_wire::error::AppErrorKind;

    #[test]
    fn parse_kind_maps_known_spellings() {
        assert_eq!(RuntimeBackend::parse_kind("acp"), ProviderKind::Acp);
        assert_eq!(
            RuntimeBackend::parse_kind("grok"),
            ProviderKind::GrokInProcess
        );
        assert_eq!(
            RuntimeBackend::parse_kind("grok_in_process"),
            ProviderKind::GrokInProcess
        );
        assert_eq!(RuntimeBackend::parse_kind("fake"), ProviderKind::Fake);
        assert_eq!(RuntimeBackend::parse_kind("other"), ProviderKind::Fake);
        assert_ne!(RuntimeBackend::parse_kind("acp"), ProviderKind::Fake);
        assert_ne!(RuntimeBackend::parse_kind("grok"), ProviderKind::Acp);
        assert_ne!(
            RuntimeBackend::parse_kind("grok_in_process"),
            ProviderKind::Fake
        );
    }

    #[test]
    fn map_err_splits_provider_and_resman() {
        assert_eq!(
            RuntimeBackend::map_err(SessionRuntimeError::Provider(ProviderError::NotFound(
                "sess-9".into()
            ))),
            BackendError::NotFound {
                session_id: "sess-9".into(),
            }
        );
        let conflict = RuntimeBackend::map_err(SessionRuntimeError::Provider(
            ProviderError::Conflict("busy".into()),
        ));
        assert!(matches!(
            conflict,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::Conflict && message == "conflict: busy"
        ));
        let resman =
            RuntimeBackend::map_err(SessionRuntimeError::Resman(ManagerError::UnknownSession(3)));
        assert!(matches!(
            resman,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::ProviderError && message.contains("unknown session 3")
        ));
    }

    #[test]
    fn push_event_maps_each_variant_and_increments_seq() {
        let mut backend = RuntimeBackend::new();
        let session = SessionId::from("sess-1");
        let mut out = Vec::new();
        backend.push_event(
            ProviderEvent::SessionReady {
                session: session.clone(),
            },
            &mut out,
        );
        backend.push_event(
            ProviderEvent::TextDelta {
                session: session.clone(),
                text: "hi".into(),
            },
            &mut out,
        );
        backend.push_event(
            ProviderEvent::TurnFinished {
                session: session.clone(),
            },
            &mut out,
        );
        backend.push_event(
            ProviderEvent::TurnFailed {
                session: session.clone(),
                message: "interrupted".into(),
            },
            &mut out,
        );
        backend.push_event(
            ProviderEvent::ApprovalRequested {
                session,
                request_id: "req-1".into(),
                tool: "shell".into(),
            },
            &mut out,
        );
        assert_eq!(out.len(), 5);
        assert_eq!(out[0].event, EventKind::SessionStatus);
        assert_eq!(out[1].event, EventKind::AgentMessageChunk);
        assert_eq!(out[2].data["status"], "finished");
        assert_eq!(out[3].data["status"], "failed");
        assert_eq!(out[4].event, EventKind::PermissionRequest);
        assert_eq!(out[0].seq, 1);
        assert_eq!(out[4].seq, 5);
    }

    #[test]
    fn get_send_interrupt_and_approval_paths() {
        let mut backend = RuntimeBackend::new();
        let started = backend
            .start(SessionStartParams {
                provider: "acp".into(),
                model: "m".into(),
                workspace: "/w".into(),
                initial_prompt: None,
            })
            .unwrap();
        let listed = backend.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, started.session_id);
        assert_eq!(listed[0].model, "m");
        assert_eq!(listed[0].workspace, "/w");
        let snap = backend.get(&started.session_id).unwrap();
        assert_eq!(snap.id, started.session_id);
        assert_eq!(snap.provider, "fake");
        assert_eq!(snap.model, "m");
        assert_eq!(snap.workspace, "/w");
        backend.send_turn(&started.session_id, "hello").unwrap();
        let events = backend.drain_events();
        assert!(events
            .iter()
            .any(|e| e.event == EventKind::AgentMessageChunk));
        let idle = backend.interrupt(&started.session_id).unwrap_err();
        assert!(matches!(
            idle,
            BackendError::Provider { kind, .. } if kind == AppErrorKind::InvalidState
        ));
        let missing = backend
            .approval_respond(&started.session_id, "req-missing", ApprovalDecision::Allow)
            .unwrap_err();
        assert!(matches!(missing, BackendError::NotFound { .. }));
        assert!(backend.get("missing").is_err());
        backend.stop(&started.session_id).unwrap();
        assert!(backend.list().is_empty());
    }

    #[test]
    fn seventh_start_exhausts_fake_resman() {
        let mut backend = RuntimeBackend::new();
        for i in 0..6 {
            backend
                .start(SessionStartParams {
                    provider: "fake".into(),
                    model: format!("m{i}"),
                    workspace: format!("/w{i}"),
                    initial_prompt: None,
                })
                .unwrap();
        }
        let err = backend
            .start(SessionStartParams {
                provider: "fake".into(),
                model: "overflow".into(),
                workspace: "/full".into(),
                initial_prompt: None,
            })
            .unwrap_err();
        assert!(matches!(
            err,
            BackendError::Provider { kind, .. } if kind == AppErrorKind::ProviderError
        ));
    }
}
