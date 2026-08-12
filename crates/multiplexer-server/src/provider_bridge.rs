//! [`SessionBackend`] over a [`ProviderAdapter`].

use std::path::PathBuf;

use multiplexer_provider::{
    FakeProvider, ModelId, ProviderAdapter, ProviderError, ProviderEvent, ProviderKind, SessionId,
    SessionStartParams as ProviderStart, TurnInput,
};
use multiplexer_wire::approval::ApprovalDecision;
use multiplexer_wire::event::{EventKind, StreamEvent};
use serde_json::json;

use crate::backend::{
    BackendError, SessionBackend, SessionSnapshot, SessionStartParams, SessionSummary,
    StartedSession,
};

/// Adapts any [`ProviderAdapter`] into the router backend.
pub struct ProviderBridge<A> {
    adapter: A,
    seq: u64,
}

impl ProviderBridge<FakeProvider> {
    pub fn fake() -> Self {
        Self::new(FakeProvider::new())
    }
}

impl<A: ProviderAdapter> ProviderBridge<A> {
    pub fn new(adapter: A) -> Self {
        Self { adapter, seq: 0 }
    }

    fn map_err(err: ProviderError) -> BackendError {
        match &err {
            ProviderError::NotFound(id) => BackendError::NotFound {
                session_id: id.clone(),
            },
            other => BackendError::Provider {
                kind: other.kind(),
                message: other.to_string(),
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

impl<A: ProviderAdapter> SessionBackend for ProviderBridge<A> {
    fn start(&mut self, params: SessionStartParams) -> Result<StartedSession, BackendError> {
        let id = self
            .adapter
            .start_session(ProviderStart {
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
        self.adapter
            .list_sessions()
            .into_iter()
            .filter_map(|id| {
                let snap = self.adapter.get_session(&id)?;
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
        let snap = self
            .adapter
            .get_session(&id)
            .ok_or_else(|| BackendError::NotFound {
                session_id: session_id.to_owned(),
            })?;
        Ok(SessionSnapshot {
            id: snap.id.to_string(),
            provider: self.adapter.kind().as_str().to_owned(),
            model: snap.model.to_string(),
            workspace: snap.workspace.to_string_lossy().into_owned(),
            initial_prompt: None,
        })
    }

    fn stop(&mut self, session_id: &str) -> Result<(), BackendError> {
        self.adapter
            .session_stop(&SessionId::from(session_id))
            .map_err(Self::map_err)
    }

    fn send_turn(&mut self, session_id: &str, text: &str) -> Result<(), BackendError> {
        self.adapter
            .send_turn(
                &SessionId::from(session_id),
                TurnInput {
                    text: text.to_owned(),
                },
            )
            .map_err(Self::map_err)
    }

    fn interrupt(&mut self, session_id: &str) -> Result<(), BackendError> {
        self.adapter
            .interrupt_turn(&SessionId::from(session_id))
            .map_err(Self::map_err)
    }

    fn approval_respond(
        &mut self,
        session_id: &str,
        request_id: &str,
        decision: ApprovalDecision,
    ) -> Result<(), BackendError> {
        self.adapter
            .approval_respond(&SessionId::from(session_id), request_id, decision)
            .map_err(Self::map_err)
    }

    fn drain_events(&mut self) -> Vec<StreamEvent> {
        let ids = self.adapter.list_sessions();
        let mut out = Vec::new();
        for id in ids {
            while let Some(ev) = self.adapter.poll_event(&id) {
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
    use multiplexer_wire::error::AppErrorKind;

    #[test]
    fn parse_kind_maps_known_spellings() {
        assert_eq!(
            ProviderBridge::<FakeProvider>::parse_kind("acp"),
            ProviderKind::Acp
        );
        assert_eq!(
            ProviderBridge::<FakeProvider>::parse_kind("grok"),
            ProviderKind::GrokInProcess
        );
        assert_eq!(
            ProviderBridge::<FakeProvider>::parse_kind("grok_in_process"),
            ProviderKind::GrokInProcess
        );
        assert_eq!(
            ProviderBridge::<FakeProvider>::parse_kind("fake"),
            ProviderKind::Fake
        );
        assert_eq!(
            ProviderBridge::<FakeProvider>::parse_kind("other"),
            ProviderKind::Fake
        );
        assert_ne!(
            ProviderBridge::<FakeProvider>::parse_kind("acp"),
            ProviderKind::Fake
        );
        assert_ne!(
            ProviderBridge::<FakeProvider>::parse_kind("grok"),
            ProviderKind::Acp
        );
        assert_ne!(
            ProviderBridge::<FakeProvider>::parse_kind("grok_in_process"),
            ProviderKind::Fake
        );
    }

    #[test]
    fn map_err_splits_not_found_from_provider_kinds() {
        assert_eq!(
            ProviderBridge::<FakeProvider>::map_err(ProviderError::NotFound("sess-9".into())),
            BackendError::NotFound {
                session_id: "sess-9".into(),
            }
        );
        let conflict =
            ProviderBridge::<FakeProvider>::map_err(ProviderError::Conflict("busy".into()));
        assert!(matches!(
            conflict,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::Conflict && message == "conflict: busy"
        ));
        let invalid =
            ProviderBridge::<FakeProvider>::map_err(ProviderError::InvalidState("idle".into()));
        assert!(matches!(
            invalid,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::InvalidState && message == "invalid state: idle"
        ));
        let provider =
            ProviderBridge::<FakeProvider>::map_err(ProviderError::Provider("down".into()));
        assert!(matches!(
            provider,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::ProviderError && message == "provider error: down"
        ));
    }

    #[test]
    fn push_event_maps_each_variant_and_increments_seq() {
        let mut bridge = ProviderBridge::fake();
        let session = SessionId::from("sess-1");
        let mut out = Vec::new();
        bridge.push_event(
            ProviderEvent::SessionReady {
                session: session.clone(),
            },
            &mut out,
        );
        bridge.push_event(
            ProviderEvent::TextDelta {
                session: session.clone(),
                text: "hi".into(),
            },
            &mut out,
        );
        bridge.push_event(
            ProviderEvent::TurnFinished {
                session: session.clone(),
            },
            &mut out,
        );
        bridge.push_event(
            ProviderEvent::TurnFailed {
                session: session.clone(),
                message: "interrupted".into(),
            },
            &mut out,
        );
        bridge.push_event(
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
        assert_ne!(out[2].data, out[3].data);
    }

    #[test]
    fn get_returns_snapshot_for_live_session() {
        let mut bridge = ProviderBridge::fake();
        let started = bridge
            .start(SessionStartParams {
                provider: "grok".into(),
                model: "grok-4".into(),
                workspace: "/ws".into(),
                initial_prompt: Some("hi".into()),
            })
            .unwrap();
        let snap = bridge.get(&started.session_id).unwrap();
        assert_eq!(snap.id, started.session_id);
        assert_eq!(snap.provider, "fake");
        assert_eq!(snap.model, "grok-4");
        assert_eq!(snap.workspace, "/ws");
        assert!(snap.initial_prompt.is_none());
        let listed = bridge.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, started.session_id);
    }
}
