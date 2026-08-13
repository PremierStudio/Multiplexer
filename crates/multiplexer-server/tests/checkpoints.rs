//! Checkpoint RPC against an installed [`CheckpointStore`].

use multiplexer_server::{CheckpointCatalog, CheckpointInfo, CheckpointStore, Server};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::error::standard;
use multiplexer_wire::error::{AppErrorKind, RpcError};
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

fn app_kind(err: &RpcError) -> &str {
    err.data.as_ref().map(|d| d.kind.as_str()).unwrap_or("")
}

fn seeded_store() -> CheckpointStore {
    let mut store = CheckpointStore::new();
    store.create("sess-a", "start");
    store.create("sess-b", "other");
    store.create("sess-a", "turn");
    store
}

#[test]
fn list_without_catalog_is_unsupported() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "sess-a" }),
    ));
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("c1".into()));
    assert_eq!(err.code, AppErrorKind::Unsupported.code());
    assert_eq!(app_kind(&err), "unsupported");
}

#[test]
fn create_without_catalog_is_unsupported() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_CREATE,
        json!({ "session_id": "sess-a", "label": "manual" }),
    ));
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("c1".into()));
    assert_eq!(err.code, AppErrorKind::Unsupported.code());
    assert_eq!(app_kind(&err), "unsupported");
}

#[test]
fn revert_without_catalog_is_unsupported() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_REVERT,
        json!({ "checkpoint_id": "cp-1" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, AppErrorKind::Unsupported.code());
    assert_eq!(app_kind(&err), "unsupported");
}

#[test]
fn install_checkpoints_enables_list() {
    let server = Server::new();
    server.install_checkpoints(seeded_store());
    let frames = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "sess-a" }),
    ));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("c1".into()));
    let rows = result["checkpoints"].as_array().expect("checkpoints");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], "cp-1");
    assert_eq!(rows[0]["label"], "start");
    assert_eq!(rows[0]["seq"], 1);
    assert_eq!(rows[1]["id"], "cp-3");
    assert_eq!(rows[1]["label"], "turn");
    assert_eq!(rows[1]["seq"], 2);
    assert!(rows[0].get("session_id").is_none());
}

#[test]
fn list_is_session_scoped() {
    let server = Server::new();
    server.install_checkpoints(seeded_store());
    let frames = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "sess-b" }),
    ));
    let rows = first_response(&frames).1["checkpoints"]
        .as_array()
        .expect("checkpoints")
        .clone();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "cp-2");
    assert_eq!(rows[0]["label"], "other");
    assert_eq!(rows[0]["seq"], 1);

    let empty = server.handle_frame(&rpc(
        "c2",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "missing" }),
    ));
    let empty_resp = first_response(&empty);
    let missing = empty_resp.1["checkpoints"].as_array().expect("checkpoints");
    assert!(missing.is_empty());
}

#[test]
fn create_appends_row_and_list_sees_it() {
    let server = Server::new();
    let mut store = CheckpointStore::new();
    store.create("sess-a", "start");
    server.install_checkpoints(store);
    let frames = server.handle_frame(&rpc(
        "n1",
        methods::CHECKPOINT_CREATE,
        json!({ "session_id": "sess-a", "label": "manual" }),
    ));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("n1".into()));
    assert_eq!(result["id"], "cp-2");
    assert_eq!(result["label"], "manual");
    assert_eq!(result["seq"], 2);

    let listed = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "sess-a" }),
    ));
    let listed_resp = first_response(&listed);
    let rows = listed_resp.1["checkpoints"]
        .as_array()
        .expect("checkpoints");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], "cp-1");
    assert_eq!(rows[0]["label"], "start");
    assert_eq!(rows[0]["seq"], 1);
    assert_eq!(rows[1]["id"], "cp-2");
    assert_eq!(rows[1]["label"], "manual");
    assert_eq!(rows[1]["seq"], 2);
}

#[test]
fn catalog_create_returns_row() {
    let mut store = CheckpointStore::new();
    let row = CheckpointCatalog::create(&mut store, "sess-a", "manual").expect("create");
    assert_eq!(
        row,
        CheckpointInfo {
            id: "cp-1".into(),
            label: "manual".into(),
            seq: 1,
            ..CheckpointInfo::default()
        }
    );
    assert_eq!(CheckpointCatalog::list(&store, "sess-a"), vec![row]);
}

#[test]
fn revert_sets_current_and_keeps_history() {
    let server = Server::new();
    server.install_checkpoints(seeded_store());
    let frames = server.handle_frame(&rpc(
        "r1",
        methods::CHECKPOINT_REVERT,
        json!({ "checkpoint_id": "cp-1" }),
    ));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("r1".into()));
    assert_eq!(result["id"], "cp-1");
    assert_eq!(result["label"], "start");
    assert_eq!(result["seq"], 1);

    let listed = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "sess-a" }),
    ));
    let listed_resp = first_response(&listed);
    let rows = listed_resp.1["checkpoints"]
        .as_array()
        .expect("checkpoints");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], "cp-1");
    assert_eq!(rows[1]["id"], "cp-3");
}

#[test]
fn revert_unknown_is_not_found() {
    let server = Server::new();
    server.install_checkpoints(seeded_store());
    let frames = server.handle_frame(&rpc(
        "r1",
        methods::CHECKPOINT_REVERT,
        json!({ "checkpoint_id": "cp-99" }),
    ));
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("r1".into()));
    assert_eq!(err.code, AppErrorKind::NotFound.code());
    assert_eq!(app_kind(&err), "not_found");
}

#[test]
fn list_and_revert_reject_invalid_params() {
    let server = Server::new();
    server.install_checkpoints(CheckpointStore::new());
    let cases = [
        (methods::CHECKPOINT_LIST, json!({})),
        (methods::CHECKPOINT_LIST, json!({ "session_id": "" })),
        (methods::CHECKPOINT_LIST, json!({ "session_id": 1 })),
        (methods::CHECKPOINT_LIST, Value::Null),
        (methods::CHECKPOINT_REVERT, json!({})),
        (methods::CHECKPOINT_REVERT, json!({ "checkpoint_id": "" })),
        (methods::CHECKPOINT_REVERT, json!({ "checkpoint_id": 1 })),
        (methods::CHECKPOINT_REVERT, Value::Null),
        (methods::CHECKPOINT_CREATE, json!({})),
        (
            methods::CHECKPOINT_CREATE,
            json!({ "session_id": "", "label": "manual" }),
        ),
        (
            methods::CHECKPOINT_CREATE,
            json!({ "session_id": "sess-a", "label": "" }),
        ),
        (
            methods::CHECKPOINT_CREATE,
            json!({ "session_id": 1, "label": "manual" }),
        ),
        (
            methods::CHECKPOINT_CREATE,
            json!({ "session_id": "sess-a", "label": 1 }),
        ),
        (
            methods::CHECKPOINT_CREATE,
            json!({ "session_id": "sess-a" }),
        ),
        (methods::CHECKPOINT_CREATE, Value::Null),
    ];
    for (method, params) in cases {
        let frames = server.handle_frame(&rpc("bad", method, params.clone()));
        let (_, err) = first_error(&frames);
        assert_eq!(err.code, standard::INVALID_PARAMS, "{method} {params}");
    }
}

#[test]
fn invalid_params_checked_before_missing_catalog() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("bad", methods::CHECKPOINT_LIST, json!({})));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, standard::INVALID_PARAMS);
}

#[test]
fn install_replaces_previous_store() {
    let server = Server::new();
    let mut first = CheckpointStore::new();
    first.create("sess-a", "old");
    server.install_checkpoints(first);
    let mut second = CheckpointStore::new();
    second.create("sess-a", "new");
    server.install_checkpoints(second);
    let frames = server.handle_frame(&rpc(
        "c1",
        methods::CHECKPOINT_LIST,
        json!({ "session_id": "sess-a" }),
    ));
    let listed_resp = first_response(&frames);
    let rows = listed_resp.1["checkpoints"]
        .as_array()
        .expect("checkpoints");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["id"], "cp-1");
    assert_eq!(rows[0]["label"], "new");
}

#[test]
fn ram_catalog_diff_is_unsupported() {
    let server = Server::new();
    server.install_checkpoints(CheckpointStore::new());
    let frames = server.handle_frame(&rpc(
        "d1",
        methods::CHECKPOINT_DIFF,
        json!({ "checkpoint_id": "cp-1" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, AppErrorKind::Unsupported.code());
}
