//! Honest stub RPC: each method succeeds instead of method-not-found.

use multiplexer_server::Server;
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::error::{standard, RpcError};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::{json, Value};

fn rpc(id: &str, method: &str, params: Value) -> String {
    encode_frame(&Message::Request(Request::new(
        Id::String(id.to_owned()),
        method,
        params,
    )))
    .expect("request encodes")
}

fn decode_all(frames: &[String]) -> Vec<Message> {
    frames
        .iter()
        .map(|f| decode_frame(f).unwrap_or_else(|e| panic!("malformed outbound frame {f:?}: {e}")))
        .collect()
}

fn first_response(frames: &[String]) -> (Id, Value) {
    for msg in decode_all(frames) {
        if let Message::Response(r) = msg {
            return (r.id, r.result);
        }
    }
    panic!("expected a success response, got {frames:?}");
}

fn first_error(frames: &[String]) -> (Id, RpcError) {
    for msg in decode_all(frames) {
        if let Message::Error(e) = msg {
            return (e.id, e.error);
        }
    }
    panic!("expected an error response, got {frames:?}");
}

#[test]
fn stub_methods_return_success_not_method_not_found() {
    let server = Server::new();
    let cases = [
        (methods::MODEL_LIST, json!({})),
        (methods::MODEL_SELECT, json!({ "model": "grok" })),
        (methods::TELEMETRY_USAGE, json!({})),
        (methods::REMOTE_LIST, json!({})),
        (methods::FS_LIST, json!({})),
    ];
    for (i, (method, params)) in cases.into_iter().enumerate() {
        let id = format!("s{i}");
        let frames = server.handle_frame(&rpc(&id, method, params));
        let (resp_id, _) = first_response(&frames);
        assert_eq!(resp_id, Id::String(id), "{method}");
        for msg in decode_all(&frames) {
            if let Message::Error(err) = msg {
                assert_ne!(
                    err.error.code,
                    standard::METHOD_NOT_FOUND,
                    "{method} must not be method-not-found"
                );
            }
        }
    }
}

#[test]
fn model_list_returns_static_catalog() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("m1", methods::MODEL_LIST, json!({})));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("m1".into()));
    assert_eq!(result["models"], json!(["grok", "grok-4.6", "fake"]));
}

#[test]
fn model_select_ok_when_model_nonempty() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "m2",
        methods::MODEL_SELECT,
        json!({ "model": "grok-4.6" }),
    ));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("m2".into()));
    assert_eq!(result, json!({ "ok": true }));
}

#[test]
fn model_select_rejects_missing_or_empty_model() {
    let server = Server::new();
    let cases = [
        json!({}),
        json!({ "model": "" }),
        json!({ "model": 1 }),
        Value::Null,
    ];
    for (i, params) in cases.into_iter().enumerate() {
        let id = format!("bad{i}");
        let frames = server.handle_frame(&rpc(&id, methods::MODEL_SELECT, params.clone()));
        let (resp_id, err) = first_error(&frames);
        assert_eq!(resp_id, Id::String(id), "{params}");
        assert_eq!(err.code, standard::INVALID_PARAMS, "{params}");
        assert_ne!(err.code, standard::METHOD_NOT_FOUND, "{params}");
    }
}

#[test]
fn telemetry_usage_is_local_snapshot() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("u1", methods::TELEMETRY_USAGE, json!({})));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("u1".into()));
    assert_eq!(result["turns"], 0);
    assert_eq!(result["tokens"], 0);
    assert_eq!(result["note"], "local snapshot only");
}

#[test]
fn remote_list_returns_this_machine() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("r1", methods::REMOTE_LIST, json!({})));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("r1".into()));
    assert_eq!(
        result["remotes"],
        json!([{ "id": "local", "kind": "local", "label": "this machine" }])
    );
}

#[test]
fn fs_list_is_honest_empty() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("f1", methods::FS_LIST, json!({})));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("f1".into()));
    assert_eq!(result["entries"], json!([]));
    assert_eq!(result["note"], "client lists via multiplexer-client");
}
