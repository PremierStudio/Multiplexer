//! Canonical provider events. Names align with wire [`EventKind`]s.

use multiplexer_wire::event::EventKind;

use crate::ids::SessionId;

/// Normalized backend event for one session.
///
/// Wire projection uses [`ProviderEvent::wire_kind`]; this enum is not the
/// JSON-RPC payload and must not change [`EventKind`] spellings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderEvent {
    /// Session is live; first event after a successful start.
    SessionReady { session: SessionId },
    /// Incremental assistant text (echoed by [`crate::FakeProvider`]).
    TextDelta { session: SessionId, text: String },
    /// Current turn completed successfully.
    TurnFinished { session: SessionId },
    /// Current turn ended in failure (including interrupt).
    TurnFailed { session: SessionId, message: String },
    /// Backend is blocked on a 4-way approval decision.
    ApprovalRequested {
        session: SessionId,
        request_id: String,
        tool: String,
    },
}

impl ProviderEvent {
    /// Session this event belongs to.
    pub fn session(&self) -> &SessionId {
        match self {
            Self::SessionReady { session }
            | Self::TextDelta { session, .. }
            | Self::TurnFinished { session }
            | Self::TurnFailed { session, .. }
            | Self::ApprovalRequested { session, .. } => session,
        }
    }

    /// Matching wire [`EventKind`] (plan/04 vocabulary). Does not serialize.
    pub fn wire_kind(&self) -> EventKind {
        match self {
            Self::SessionReady { .. } => EventKind::SessionStatus,
            Self::TextDelta { .. } => EventKind::AgentMessageChunk,
            Self::TurnFinished { .. } | Self::TurnFailed { .. } => EventKind::TurnStatus,
            Self::ApprovalRequested { .. } => EventKind::PermissionRequest,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess(id: &str) -> SessionId {
        SessionId(id.to_owned())
    }

    #[test]
    fn wire_kinds_match_event_vocabulary() {
        let s = sess("sess-1");
        assert_eq!(
            ProviderEvent::SessionReady { session: s.clone() }.wire_kind(),
            EventKind::SessionStatus
        );
        assert_eq!(
            ProviderEvent::TextDelta {
                session: s.clone(),
                text: "x".into(),
            }
            .wire_kind(),
            EventKind::AgentMessageChunk
        );
        assert_eq!(
            ProviderEvent::TurnFinished { session: s.clone() }.wire_kind(),
            EventKind::TurnStatus
        );
        assert_eq!(
            ProviderEvent::TurnFailed {
                session: s.clone(),
                message: "interrupted".into(),
            }
            .wire_kind(),
            EventKind::TurnStatus
        );
        assert_eq!(
            ProviderEvent::ApprovalRequested {
                session: s,
                request_id: "r".into(),
                tool: "shell".into(),
            }
            .wire_kind(),
            EventKind::PermissionRequest
        );
    }

    #[test]
    fn session_accessor_reads_each_variant() {
        let s = sess("sess-7");
        assert_eq!(
            ProviderEvent::SessionReady { session: s.clone() }
                .session()
                .0,
            "sess-7"
        );
        assert_eq!(
            ProviderEvent::TextDelta {
                session: s.clone(),
                text: "d".into(),
            }
            .session()
            .0,
            "sess-7"
        );
        assert_eq!(
            ProviderEvent::TurnFinished { session: s.clone() }
                .session()
                .0,
            "sess-7"
        );
        assert_eq!(
            ProviderEvent::TurnFailed {
                session: s.clone(),
                message: "e".into(),
            }
            .session()
            .0,
            "sess-7"
        );
        assert_eq!(
            ProviderEvent::ApprovalRequested {
                session: s,
                request_id: "r".into(),
                tool: "t".into(),
            }
            .session()
            .0,
            "sess-7"
        );
    }

    #[test]
    fn variants_are_not_equal() {
        let s = sess("sess-1");
        let ready = ProviderEvent::SessionReady { session: s.clone() };
        let delta = ProviderEvent::TextDelta {
            session: s.clone(),
            text: "x".into(),
        };
        let finished = ProviderEvent::TurnFinished { session: s.clone() };
        let failed = ProviderEvent::TurnFailed {
            session: s.clone(),
            message: "interrupted".into(),
        };
        let approval = ProviderEvent::ApprovalRequested {
            session: s,
            request_id: "r".into(),
            tool: "t".into(),
        };
        assert_ne!(ready, delta);
        assert_ne!(finished, failed);
        assert_ne!(delta, finished);
        assert_ne!(failed, approval);
        assert_ne!(ready, approval);
        assert_ne!(
            ProviderEvent::TextDelta {
                session: sess("sess-1"),
                text: "a".into(),
            },
            ProviderEvent::TextDelta {
                session: sess("sess-1"),
                text: "b".into(),
            }
        );
        assert_ne!(
            ProviderEvent::TurnFailed {
                session: sess("sess-1"),
                message: "interrupted".into(),
            },
            ProviderEvent::TurnFailed {
                session: sess("sess-1"),
                message: "other".into(),
            }
        );
    }
}
