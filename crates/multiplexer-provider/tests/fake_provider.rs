//! Integration tests for [`FakeProvider`] and the [`ProviderAdapter`] seam.

use std::collections::HashSet;
use std::path::PathBuf;

use multiplexer_provider::{
    FakeProvider, ModelId, ProviderAdapter, ProviderError, ProviderEvent, ProviderKind, SessionId,
    SessionStartParams, TurnInput,
};
use multiplexer_wire::approval::ApprovalDecision;
use multiplexer_wire::error::AppErrorKind;
use multiplexer_wire::event::EventKind;
use proptest::prelude::*;

fn workspace() -> PathBuf {
    PathBuf::from("C:\\work\\demo")
}

fn params(prompt: Option<&str>) -> SessionStartParams {
    SessionStartParams {
        provider: ProviderKind::Fake,
        model: ModelId("grok-test".into()),
        workspace: workspace(),
        initial_prompt: prompt.map(str::to_string),
        resume: None,
    }
}

fn turn(text: &str) -> TurnInput {
    TurnInput {
        text: text.to_owned(),
    }
}

fn drain(p: &FakeProvider, session: &SessionId) -> Vec<ProviderEvent> {
    let mut out = Vec::new();
    while let Some(event) = p.poll_event(session) {
        out.push(event);
    }
    out
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn fake_and_trait_object_are_send_sync() {
    assert_send_sync::<FakeProvider>();
    assert_send_sync::<Box<dyn ProviderAdapter>>();
}

#[test]
fn kind_is_fake() {
    let p = FakeProvider::new();
    assert_eq!(p.kind(), ProviderKind::Fake);
    assert_eq!(p.kind().as_str(), "fake");
}

#[test]
fn start_session_assigns_incrementing_ids() {
    let p = FakeProvider::new();
    let a = p.start_session(params(None)).expect("start a");
    let b = p.start_session(params(None)).expect("start b");
    assert_eq!(a.0, "sess-1");
    assert_eq!(b.0, "sess-2");
    assert_ne!(a, b);
}

#[test]
fn start_without_prompt_emits_only_session_ready() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    assert_eq!(
        p.poll_event(&id),
        Some(ProviderEvent::SessionReady {
            session: id.clone()
        })
    );
    assert_eq!(p.poll_event(&id), None);
    let snap = p.get_session(&id).expect("snapshot");
    assert!(!snap.running);
    assert_eq!(snap.model.0, "grok-test");
    assert_eq!(snap.workspace, workspace());
}

#[test]
fn start_with_prompt_echoes_then_finishes() {
    let p = FakeProvider::new();
    let id = p.start_session(params(Some("hello-agent"))).expect("start");
    let events = drain(&p, &id);
    assert_eq!(
        events,
        vec![
            ProviderEvent::SessionReady {
                session: id.clone()
            },
            ProviderEvent::TextDelta {
                session: id.clone(),
                text: "hello-agent".into(),
            },
            ProviderEvent::TurnFinished {
                session: id.clone()
            },
        ]
    );
    assert_eq!(events[1].wire_kind(), EventKind::AgentMessageChunk);
    assert_eq!(events[2].wire_kind(), EventKind::TurnStatus);
    assert_eq!(events[0].wire_kind(), EventKind::SessionStatus);
}

#[test]
fn start_with_empty_prompt_still_emits_delta() {
    let p = FakeProvider::new();
    let id = p.start_session(params(Some(""))).expect("start");
    let events = drain(&p, &id);
    assert_eq!(events.len(), 3);
    match &events[1] {
        ProviderEvent::TextDelta { text, .. } => assert_eq!(text, ""),
        other => panic!("expected empty TextDelta, got {other:?}"),
    }
}

#[test]
fn resume_does_not_reuse_ids() {
    let p = FakeProvider::new();
    let mut first = params(None);
    first.resume = Some("cursor-1".into());
    let a = p.start_session(first).expect("start");
    assert_eq!(a.0, "sess-1");
}

#[test]
fn send_turn_echoes_text_then_finishes() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let _ = drain(&p, &id);
    p.send_turn(&id, turn("prompt-text")).expect("turn");
    assert_eq!(
        drain(&p, &id),
        vec![
            ProviderEvent::TextDelta {
                session: id.clone(),
                text: "prompt-text".into(),
            },
            ProviderEvent::TurnFinished {
                session: id.clone()
            },
        ]
    );
    assert!(!p.get_session(&id).expect("snap").running);
}

#[test]
fn send_turn_unknown_session_is_not_found() {
    let p = FakeProvider::new();
    let err = p
        .send_turn(&SessionId("sess-99".into()), turn("x"))
        .expect_err("missing");
    assert_eq!(err, ProviderError::NotFound("session sess-99".into()));
    assert_eq!(err.kind(), AppErrorKind::NotFound);
}

#[test]
fn send_turn_while_running_is_conflict() {
    let p = FakeProvider::new();
    p.block_next_turn();
    let id = p.start_session(params(None)).expect("start");
    p.send_turn(&id, turn("first")).expect("blocked turn");
    assert!(p.get_session(&id).expect("snap").running);
    let err = p.send_turn(&id, turn("second")).expect_err("conflict");
    assert_eq!(err, ProviderError::Conflict("turn already running".into()));
    assert_eq!(err.kind(), AppErrorKind::Conflict);
    let events = drain(&p, &id);
    assert!(
        matches!(events.last(), Some(ProviderEvent::TextDelta { text, .. }) if text == "first")
    );
    assert!(!events
        .iter()
        .any(|e| matches!(e, ProviderEvent::TurnFinished { .. })));
}

#[test]
fn block_next_turn_is_one_shot() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let _ = drain(&p, &id);
    p.block_next_turn();
    p.send_turn(&id, turn("held")).expect("held");
    p.complete_turn(&id).expect("complete");
    p.send_turn(&id, turn("free")).expect("free");
    let events = drain(&p, &id);
    assert_eq!(
        events,
        vec![
            ProviderEvent::TextDelta {
                session: id.clone(),
                text: "held".into(),
            },
            ProviderEvent::TurnFinished {
                session: id.clone()
            },
            ProviderEvent::TextDelta {
                session: id.clone(),
                text: "free".into(),
            },
            ProviderEvent::TurnFinished {
                session: id.clone()
            },
        ]
    );
}

#[test]
fn initial_prompt_respects_block_next_turn() {
    let p = FakeProvider::new();
    p.block_next_turn();
    let id = p.start_session(params(Some("boot"))).expect("start");
    assert!(p.get_session(&id).expect("snap").running);
    let events = drain(&p, &id);
    assert_eq!(events.len(), 2);
    assert!(matches!(events[0], ProviderEvent::SessionReady { .. }));
    assert!(matches!(
        &events[1],
        ProviderEvent::TextDelta { text, .. } if text == "boot"
    ));
}

#[test]
fn complete_turn_finishes_blocked_turn() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let _ = drain(&p, &id);
    p.block_next_turn();
    p.send_turn(&id, turn("work")).expect("send");
    p.complete_turn(&id).expect("complete");
    assert!(!p.get_session(&id).expect("snap").running);
    assert_eq!(
        p.poll_event(&id),
        Some(ProviderEvent::TextDelta {
            session: id.clone(),
            text: "work".into(),
        })
    );
    assert_eq!(
        p.poll_event(&id),
        Some(ProviderEvent::TurnFinished {
            session: id.clone()
        })
    );
}

#[test]
fn complete_turn_without_running_is_invalid_state() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let err = p.complete_turn(&id).expect_err("idle");
    assert_eq!(err, ProviderError::InvalidState("no running turn".into()));
    assert_eq!(err.kind(), AppErrorKind::InvalidState);
}

#[test]
fn complete_turn_unknown_session_is_not_found() {
    let p = FakeProvider::new();
    let err = p
        .complete_turn(&SessionId("sess-1".into()))
        .expect_err("missing");
    assert_eq!(err, ProviderError::NotFound("session sess-1".into()));
}

#[test]
fn interrupt_turn_fails_blocked_turn() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let _ = drain(&p, &id);
    p.block_next_turn();
    p.send_turn(&id, turn("work")).expect("send");
    p.interrupt_turn(&id).expect("interrupt");
    assert!(!p.get_session(&id).expect("snap").running);
    let events = drain(&p, &id);
    assert_eq!(
        events,
        vec![
            ProviderEvent::TextDelta {
                session: id.clone(),
                text: "work".into(),
            },
            ProviderEvent::TurnFailed {
                session: id.clone(),
                message: "interrupted".into(),
            },
        ]
    );
    assert_eq!(events[1].wire_kind(), EventKind::TurnStatus);
    p.send_turn(&id, turn("after")).expect("next turn");
}

#[test]
fn interrupt_turn_without_running_is_invalid_state() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let err = p.interrupt_turn(&id).expect_err("idle");
    assert_eq!(err, ProviderError::InvalidState("no running turn".into()));
    assert_eq!(err.kind(), AppErrorKind::InvalidState);
}

#[test]
fn interrupt_turn_unknown_session_is_not_found() {
    let p = FakeProvider::new();
    let err = p
        .interrupt_turn(&SessionId("gone".into()))
        .expect_err("missing");
    assert_eq!(err, ProviderError::NotFound("session gone".into()));
}

#[test]
fn complete_after_interrupt_is_invalid_state() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    p.block_next_turn();
    p.send_turn(&id, turn("x")).expect("send");
    p.interrupt_turn(&id).expect("interrupt");
    let err = p.complete_turn(&id).expect_err("already interrupted");
    assert_eq!(err, ProviderError::InvalidState("no running turn".into()));
}

#[test]
fn interrupt_after_complete_is_invalid_state() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    p.block_next_turn();
    p.send_turn(&id, turn("x")).expect("send");
    p.complete_turn(&id).expect("complete");
    let err = p.interrupt_turn(&id).expect_err("already complete");
    assert_eq!(err, ProviderError::InvalidState("no running turn".into()));
}

#[test]
fn approval_respond_unknown_request_is_not_found() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let err = p
        .approval_respond(&id, "req-missing", ApprovalDecision::Allow)
        .expect_err("missing request");
    assert_eq!(
        err,
        ProviderError::NotFound("approval request req-missing".into())
    );
    assert_eq!(err.kind(), AppErrorKind::NotFound);
}

#[test]
fn approval_respond_unknown_session_is_not_found() {
    let p = FakeProvider::new();
    let err = p
        .approval_respond(&SessionId("sess-1".into()), "req-1", ApprovalDecision::Deny)
        .expect_err("missing session");
    assert_eq!(err, ProviderError::NotFound("session sess-1".into()));
}

#[test]
fn approval_respond_records_known_request() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let _ = drain(&p, &id);
    p.request_approval(&id, "req-1", "shell").expect("request");
    let requested = p.poll_event(&id);
    assert_eq!(
        requested,
        Some(ProviderEvent::ApprovalRequested {
            session: id.clone(),
            request_id: "req-1".into(),
            tool: "shell".into(),
        })
    );
    assert_eq!(
        requested.as_ref().map(ProviderEvent::wire_kind),
        Some(EventKind::PermissionRequest)
    );
    assert_eq!(p.poll_event(&id), None);
    p.approval_respond(&id, "req-1", ApprovalDecision::AllowOnce)
        .expect("respond");
    assert_eq!(
        p.last_approval(&id),
        Some(("req-1".into(), ApprovalDecision::AllowOnce))
    );
    let err = p
        .approval_respond(&id, "req-1", ApprovalDecision::Allow)
        .expect_err("consumed");
    assert_eq!(
        err,
        ProviderError::NotFound("approval request req-1".into())
    );
}

#[test]
fn approval_decisions_are_distinct() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let decisions = [
        ApprovalDecision::Allow,
        ApprovalDecision::Deny,
        ApprovalDecision::AllowOnce,
        ApprovalDecision::AllowAlways,
    ];
    for (i, decision) in decisions.into_iter().enumerate() {
        let req = format!("req-{i}");
        p.request_approval(&id, &req, "fs").expect("request");
        p.approval_respond(&id, &req, decision).expect("respond");
        assert_eq!(p.last_approval(&id), Some((req, decision)));
    }
}

#[test]
fn two_pending_approvals_are_independent() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    p.request_approval(&id, "a", "shell").expect("a");
    p.request_approval(&id, "b", "fs").expect("b");
    p.approval_respond(&id, "a", ApprovalDecision::Deny)
        .expect("deny a");
    p.approval_respond(&id, "b", ApprovalDecision::AllowAlways)
        .expect("allow b");
    assert_eq!(
        p.last_approval(&id),
        Some(("b".into(), ApprovalDecision::AllowAlways))
    );
}

#[test]
fn request_approval_unknown_session_is_not_found() {
    let p = FakeProvider::new();
    let err = p
        .request_approval(&SessionId("sess-1".into()), "r", "t")
        .expect_err("missing");
    assert_eq!(err, ProviderError::NotFound("session sess-1".into()));
}

#[test]
fn session_stop_removes_session() {
    let p = FakeProvider::new();
    let id = p.start_session(params(Some("x"))).expect("start");
    p.session_stop(&id).expect("stop");
    assert!(p.get_session(&id).is_none());
    assert!(p.poll_event(&id).is_none());
    assert!(p.list_sessions().is_empty());
    assert_eq!(
        p.send_turn(&id, turn("y")).expect_err("gone"),
        ProviderError::NotFound("session sess-1".into())
    );
    assert_eq!(
        p.interrupt_turn(&id).expect_err("gone"),
        ProviderError::NotFound("session sess-1".into())
    );
    assert_eq!(
        p.approval_respond(&id, "r", ApprovalDecision::Allow)
            .expect_err("gone"),
        ProviderError::NotFound("session sess-1".into())
    );
    assert_eq!(
        p.session_stop(&id).expect_err("already gone"),
        ProviderError::NotFound("session sess-1".into())
    );
    assert_eq!(
        p.complete_turn(&id).expect_err("gone"),
        ProviderError::NotFound("session sess-1".into())
    );
    assert!(p.last_approval(&id).is_none());
}

#[test]
fn session_stop_unknown_is_not_found() {
    let p = FakeProvider::new();
    let err = p
        .session_stop(&SessionId("sess-1".into()))
        .expect_err("missing");
    assert_eq!(err, ProviderError::NotFound("session sess-1".into()));
}

#[test]
fn ids_do_not_reuse_after_stop() {
    let p = FakeProvider::new();
    let a = p.start_session(params(None)).expect("a");
    let b = p.start_session(params(None)).expect("b");
    p.session_stop(&a).expect("stop a");
    p.session_stop(&b).expect("stop b");
    let c = p.start_session(params(None)).expect("c");
    assert_eq!(a.0, "sess-1");
    assert_eq!(b.0, "sess-2");
    assert_eq!(c.0, "sess-3");
    assert_eq!(p.list_sessions(), vec![c]);
}

#[test]
fn list_sessions_is_insertion_order_without_stopped() {
    let p = FakeProvider::new();
    let a = p.start_session(params(None)).expect("a");
    let b = p.start_session(params(None)).expect("b");
    let c = p.start_session(params(None)).expect("c");
    assert_eq!(p.list_sessions(), vec![a.clone(), b.clone(), c.clone()]);
    p.session_stop(&b).expect("stop b");
    assert_eq!(p.list_sessions(), vec![a, c]);
}

#[test]
fn poll_event_is_fifo_per_session() {
    let p = FakeProvider::new();
    let a = p.start_session(params(Some("alpha"))).expect("a");
    let b = p.start_session(params(Some("beta"))).expect("b");
    assert_eq!(
        p.poll_event(&a),
        Some(ProviderEvent::SessionReady { session: a.clone() })
    );
    assert_eq!(
        p.poll_event(&b),
        Some(ProviderEvent::SessionReady { session: b.clone() })
    );
    assert_eq!(
        p.poll_event(&a),
        Some(ProviderEvent::TextDelta {
            session: a.clone(),
            text: "alpha".into(),
        })
    );
    assert_eq!(
        p.poll_event(&b),
        Some(ProviderEvent::TextDelta {
            session: b.clone(),
            text: "beta".into(),
        })
    );
    assert_eq!(
        p.poll_event(&a),
        Some(ProviderEvent::TurnFinished { session: a })
    );
    assert_eq!(
        p.poll_event(&b),
        Some(ProviderEvent::TurnFinished { session: b })
    );
}

#[test]
fn poll_event_unknown_session_is_none() {
    let p = FakeProvider::new();
    assert!(p.poll_event(&SessionId("sess-1".into())).is_none());
}

#[test]
fn get_session_unknown_is_none() {
    let p = FakeProvider::new();
    assert!(p.get_session(&SessionId("sess-1".into())).is_none());
}

#[test]
fn get_session_preserves_model_and_workspace() {
    let p = FakeProvider::new();
    let mut start = params(None);
    start.model = ModelId("other-model".into());
    start.workspace = PathBuf::from("D:\\ws\\other");
    start.provider = ProviderKind::GrokInProcess;
    let id = p.start_session(start).expect("start");
    let snap = p.get_session(&id).expect("snap");
    assert_eq!(snap.id, id);
    assert_eq!(snap.model.0, "other-model");
    assert_eq!(snap.workspace, PathBuf::from("D:\\ws\\other"));
    assert!(!snap.running);
}

#[test]
fn session_stop_while_running_discards_session() {
    let p = FakeProvider::new();
    p.block_next_turn();
    let id = p.start_session(params(Some("live"))).expect("start");
    assert!(p.get_session(&id).expect("snap").running);
    p.session_stop(&id).expect("stop");
    assert!(p.get_session(&id).is_none());
}

#[test]
fn clone_shares_session_table() {
    let p = FakeProvider::new();
    let q = p.clone();
    let id = p.start_session(params(None)).expect("start");
    assert_eq!(q.list_sessions(), vec![id.clone()]);
    q.session_stop(&id).expect("stop via clone");
    assert!(p.list_sessions().is_empty());
}

#[test]
fn default_matches_new() {
    let p = FakeProvider::default();
    assert!(p.list_sessions().is_empty());
    assert_eq!(p.start_session(params(None)).unwrap().0, "sess-1");
}

#[test]
fn two_providers_have_independent_counters() {
    let a = FakeProvider::new();
    let b = FakeProvider::new();
    assert_eq!(a.start_session(params(None)).unwrap().0, "sess-1");
    assert_eq!(b.start_session(params(None)).unwrap().0, "sess-1");
}

#[test]
fn trait_object_can_drive_a_turn() {
    let fake = FakeProvider::new();
    let p: &dyn ProviderAdapter = &fake;
    let id = p.start_session(params(None)).expect("start");
    p.send_turn(&id, turn("via-trait")).expect("turn");
    assert_eq!(p.kind(), ProviderKind::Fake);
    assert_eq!(p.list_sessions().len(), 1);
    assert!(p.get_session(&id).is_some());
    let mut saw_delta = false;
    while let Some(event) = p.poll_event(&id) {
        if let ProviderEvent::TextDelta { text, session } = event {
            assert_eq!(text, "via-trait");
            assert_eq!(session, id);
            saw_delta = true;
        }
    }
    assert!(saw_delta);
    p.session_stop(&id).expect("stop");
}

#[test]
fn last_approval_none_before_any_and_unknown() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    assert!(p.last_approval(&id).is_none());
    assert!(p.last_approval(&SessionId("nope".into())).is_none());
}

#[test]
fn event_session_accessor_matches_owner() {
    let p = FakeProvider::new();
    let id = p.start_session(params(Some("z"))).expect("start");
    for event in drain(&p, &id) {
        assert_eq!(event.session(), &id);
    }
}

#[test]
fn block_next_twice_still_blocks_only_one_turn() {
    let p = FakeProvider::new();
    let id = p.start_session(params(None)).expect("start");
    let _ = drain(&p, &id);
    p.block_next_turn();
    p.block_next_turn();
    p.send_turn(&id, turn("one")).expect("one");
    assert!(p.get_session(&id).expect("snap").running);
    p.complete_turn(&id).expect("done");
    p.send_turn(&id, turn("two")).expect("two");
    assert!(!p.get_session(&id).expect("snap").running);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn start_n_sessions_unique_ids_and_list_len(n in 1usize..=32) {
        let p = FakeProvider::new();
        let mut ids = Vec::new();
        for i in 0..n {
            let id = p.start_session(params(None)).expect("start");
            prop_assert_eq!(&id.0, &format!("sess-{}", i + 1));
            ids.push(id);
        }
        let unique: HashSet<String> = ids.iter().map(|s| s.0.clone()).collect();
        prop_assert_eq!(unique.len(), n);
        let listed = p.list_sessions();
        prop_assert_eq!(listed.len(), n);
        prop_assert_eq!(listed, ids);
    }
}
