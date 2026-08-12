//! SessionRuntime wires provider, resman, and checkpoints on start/stop.

use std::path::PathBuf;

use multiplexer_core::{SessionRuntime, SessionRuntimeError};
use multiplexer_provider::{
    ModelId, ProviderAdapter, ProviderError, ProviderKind, SessionId, SessionStartParams,
};
use multiplexer_resman::SessionId as ResmanSessionId;
use proptest::prelude::*;

fn params() -> SessionStartParams {
    SessionStartParams {
        provider: ProviderKind::Fake,
        model: ModelId::from("grok-test"),
        workspace: PathBuf::from("/work"),
        initial_prompt: None,
        resume: None,
    }
}

#[test]
fn default_matches_new() {
    let a = SessionRuntime::default();
    let b = SessionRuntime::new();
    assert_eq!(a.provider().list_sessions().len(), 0);
    assert_eq!(b.provider().list_sessions().len(), 0);
    assert_eq!(a.resman().session_count(), 0);
    assert_eq!(b.resman().session_count(), 0);
}

#[test]
fn start_wires_provider_resman_and_checkpoint() {
    let mut rt = SessionRuntime::new();
    let id = rt.start(params()).expect("start");
    assert_eq!(id.as_str(), "sess-1");
    assert_eq!(rt.provider().list_sessions().len(), 1);
    assert_eq!(rt.provider().list_sessions(), vec![id.clone()]);
    assert_eq!(rt.resman().session_count(), 1);
    assert_eq!(
        rt.resman()
            .alloc_of(ResmanSessionId(1))
            .unwrap()
            .cores
            .len(),
        1
    );
    assert!(rt.resman().session_alive(ResmanSessionId(1)).unwrap());
    let cps = rt.checkpoints().list(id.as_str());
    assert_eq!(cps.len(), 1);
    assert_eq!(cps[0].label, "start");
    assert_eq!(cps[0].session_id, id.as_str());
    assert_eq!(cps[0].seq, 1);
    assert_eq!(
        rt.checkpoints().current(id.as_str()).unwrap().as_str(),
        "cp-1"
    );
}

#[test]
fn stop_releases_provider_and_resman_keeps_checkpoint() {
    let mut rt = SessionRuntime::new();
    let id = rt.start(params()).expect("start");
    rt.stop(&id).expect("stop");
    assert_eq!(rt.provider().list_sessions().len(), 0);
    assert_eq!(rt.resman().session_count(), 0);
    assert_eq!(rt.checkpoints().list(id.as_str()).len(), 1);
    assert_eq!(rt.checkpoints().list(id.as_str())[0].label, "start");
}

#[test]
fn second_start_gets_next_ids() {
    let mut rt = SessionRuntime::new();
    let a = rt.start(params()).expect("a");
    let b = rt.start(params()).expect("b");
    assert_eq!(a.as_str(), "sess-1");
    assert_eq!(b.as_str(), "sess-2");
    assert_eq!(rt.provider().list_sessions().len(), 2);
    assert_eq!(rt.resman().session_count(), 2);
    assert!(rt.resman().alloc_of(ResmanSessionId(1)).is_some());
    assert!(rt.resman().alloc_of(ResmanSessionId(2)).is_some());
    assert!(rt.resman().alloc_of(ResmanSessionId(0)).is_none());
    assert_eq!(rt.checkpoints().list(a.as_str()).len(), 1);
    assert_eq!(rt.checkpoints().list(b.as_str()).len(), 1);
    assert_eq!(rt.checkpoints().list(b.as_str())[0].id.as_str(), "cp-2");
}

#[test]
fn stop_unknown_is_provider_not_found() {
    let mut rt = SessionRuntime::new();
    let err = rt
        .stop(&SessionId::from("sess-missing"))
        .expect_err("missing");
    assert!(matches!(
        err,
        SessionRuntimeError::Provider(ProviderError::NotFound(_))
    ));
    assert_eq!(err.to_string(), "not found: session sess-missing");
    assert_eq!(rt.resman().session_count(), 0);
}

#[test]
fn stop_twice_is_not_found() {
    let mut rt = SessionRuntime::new();
    let id = rt.start(params()).expect("start");
    rt.stop(&id).expect("first");
    assert!(rt.stop(&id).is_err());
    assert_eq!(rt.provider().list_sessions().len(), 0);
    assert_eq!(rt.resman().session_count(), 0);
}

#[test]
fn exhausted_cores_rolls_back_provider() {
    let mut rt = SessionRuntime::new();
    for _ in 0..6 {
        rt.start(params()).expect("allocate");
    }
    assert_eq!(rt.provider().list_sessions().len(), 6);
    assert_eq!(rt.resman().session_count(), 6);
    let err = rt.start(params()).expect_err("no cores");
    assert!(matches!(err, SessionRuntimeError::Resman(_)));
    assert!(
        err.to_string().contains("not enough free cores")
            || err.to_string().contains("resman")
            || err.to_string().contains("need")
    );
    assert_eq!(rt.provider().list_sessions().len(), 6);
    assert_eq!(rt.resman().session_count(), 6);
}

#[test]
fn error_variants_are_distinct() {
    let provider = SessionRuntimeError::from(ProviderError::NotFound("x".into()));
    let resman = SessionRuntimeError::from(multiplexer_resman::ManagerError::UnknownSession(1));
    assert_ne!(provider, resman);
    assert_eq!(provider.to_string(), "not found: x");
    assert_eq!(resman.to_string(), "unknown session 1");
}

proptest! {
    #[test]
    fn n_starts_then_stops_leave_empty_live_tables(n in 1usize..=6) {
        let mut rt = SessionRuntime::new();
        let mut ids = Vec::new();
        for _ in 0..n {
            ids.push(rt.start(params()).unwrap());
        }
        prop_assert_eq!(rt.provider().list_sessions().len(), n);
        prop_assert_eq!(rt.resman().session_count(), n);
        for id in &ids {
            prop_assert_eq!(rt.checkpoints().list(id.as_str()).len(), 1);
        }
        for id in &ids {
            rt.stop(id).unwrap();
        }
        prop_assert_eq!(rt.provider().list_sessions().len(), 0);
        prop_assert_eq!(rt.resman().session_count(), 0);
        for id in &ids {
            prop_assert_eq!(rt.checkpoints().list(id.as_str()).len(), 1);
            prop_assert_eq!(&rt.checkpoints().list(id.as_str())[0].label, "start");
        }
    }
}
