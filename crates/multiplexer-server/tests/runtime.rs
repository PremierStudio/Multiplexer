//! RuntimeBackend composes provider + resman + checkpoints on session.start.

use multiplexer_provider::ProviderAdapter;
use multiplexer_resman::SessionId as ResmanSessionId;
use multiplexer_server::{RuntimeBackend, Server, SessionBackend, SessionStartParams};
use multiplexer_wire::codec::encode_frame;
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::{json, Value};

fn params() -> SessionStartParams {
    SessionStartParams {
        provider: "fake".into(),
        model: "grok-test".into(),
        workspace: "/work".into(),
        initial_prompt: None,
    }
}

#[test]
fn start_composes_provider_resman_and_checkpoint() {
    let mut backend = RuntimeBackend::new();
    let started = backend.start(params()).expect("start");
    assert_eq!(started.session_id, "sess-1");
    let rt = backend.runtime();
    assert_eq!(rt.provider().list_sessions().len(), 1);
    assert_eq!(rt.resman().session_count(), 1);
    assert_eq!(
        rt.resman()
            .alloc_of(ResmanSessionId(1))
            .unwrap()
            .cores
            .len(),
        1
    );
    assert_eq!(rt.checkpoints().list(&started.session_id).len(), 1);
    assert_eq!(rt.checkpoints().list(&started.session_id)[0].label, "start");
}

#[test]
fn stop_releases_provider_and_resman() {
    let mut backend = RuntimeBackend::new();
    let started = backend.start(params()).expect("start");
    backend.stop(&started.session_id).expect("stop");
    let rt = backend.runtime();
    assert_eq!(rt.provider().list_sessions().len(), 0);
    assert_eq!(rt.resman().session_count(), 0);
    assert_eq!(rt.checkpoints().list(&started.session_id).len(), 1);
}

#[test]
fn default_backend_starts_empty() {
    let backend = RuntimeBackend::default();
    assert!(backend.list().is_empty());
    assert_eq!(backend.runtime().resman().session_count(), 0);
}

#[test]
fn server_with_runtime_session_start_rpc() {
    let server = Server::with_runtime();
    let req = encode_frame(&Message::Request(Request::new(
        Id::String("1".into()),
        methods::SESSION_START,
        json!({
            "provider": "fake",
            "model": "grok-test",
            "workspace": "/work",
        }),
    )))
    .expect("encode");
    let frames = server.handle_frame(&req);
    let value: Value = serde_json::from_str(&frames[0]).expect("json");
    assert_eq!(value["result"]["session_id"], "sess-1");
}
