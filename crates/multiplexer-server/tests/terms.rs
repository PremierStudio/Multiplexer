//! Terminal RPC contract tests.

use std::path::PathBuf;

use multiplexer_server::Server;
use multiplexer_terminal::{TerminalHub, TerminalId};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::error::{standard, AppErrorKind, RpcError};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use proptest::prelude::*;
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

fn wired() -> (Server, multiplexer_terminal::TerminalWatch) {
    let hub = TerminalHub::new();
    let watch = hub.watch();
    let server = Server::new();
    server.install_terminals(hub);
    (server, watch)
}

fn create(server: &Server, id: &str, cols: u16, rows: u16, cwd: Option<&str>) -> String {
    let mut params = json!({ "cols": cols, "rows": rows });
    if let Some(cwd) = cwd {
        params["cwd"] = json!(cwd);
    }
    let frames = server.handle_frame(&rpc(id, methods::TERMINAL_CREATE, params));
    let (resp_id, result) = first_response(&frames);
    assert_eq!(resp_id, Id::String(id.to_owned()));
    result["id"].as_str().expect("id string").to_owned()
}

#[test]
fn create_returns_sequential_ids() {
    let (server, _) = wired();
    assert_eq!(create(&server, "c1", 80, 24, Some("/work")), "term-1");
    assert_eq!(create(&server, "c2", 100, 30, Some("/other")), "term-2");
}

#[test]
fn create_records_spec_and_defaults_cwd() {
    let (server, watch) = wired();
    let with_cwd = create(&server, "c1", 132, 43, Some(r"C:\proj"));
    let default_cwd = create(&server, "c2", 80, 24, None);

    let snap = watch
        .get(&TerminalId::from(with_cwd.as_str()))
        .expect("created");
    assert_eq!(snap.spec.cols, 132);
    assert_eq!(snap.spec.rows, 43);
    assert_eq!(snap.spec.cwd, PathBuf::from(r"C:\proj"));
    assert!(snap.alive);

    let snap = watch
        .get(&TerminalId::from(default_cwd.as_str()))
        .expect("created");
    assert_eq!(snap.spec.cols, 80);
    assert_eq!(snap.spec.rows, 24);
    assert_eq!(snap.spec.cwd, PathBuf::from("."));
}

#[test]
fn list_starts_empty_then_shows_live_rows() {
    let (server, _) = wired();
    let frames = server.handle_frame(&rpc("l0", methods::TERMINAL_LIST, json!({})));
    let (_, result) = first_response(&frames);
    assert_eq!(result["terminals"], json!([]));

    let a = create(&server, "c1", 80, 24, None);
    let b = create(&server, "c2", 40, 12, None);
    let frames = server.handle_frame(&rpc("l1", methods::TERMINAL_LIST, Value::Null));
    let (_, result) = first_response(&frames);
    assert_eq!(
        result["terminals"],
        json!([
            { "id": a, "alive": true },
            { "id": b, "alive": true },
        ])
    );
}

#[test]
fn input_appends_utf8_bytes() {
    let (server, watch) = wired();
    let id = create(&server, "c1", 80, 24, None);
    let frames = server.handle_frame(&rpc(
        "i1",
        methods::TERMINAL_INPUT,
        json!({ "id": id, "data": "ab" }),
    ));
    assert_eq!(first_response(&frames).1, json!({}));
    let frames = server.handle_frame(&rpc(
        "i2",
        methods::TERMINAL_INPUT,
        json!({ "id": id, "data": "cd" }),
    ));
    assert_eq!(first_response(&frames).1, json!({}));
    let frames = server.handle_frame(&rpc(
        "i3",
        methods::TERMINAL_INPUT,
        json!({ "id": id, "data": "" }),
    ));
    assert_eq!(first_response(&frames).1, json!({}));

    assert_eq!(
        watch
            .get(&TerminalId::from(id.as_str()))
            .expect("snap")
            .input,
        b"abcd"
    );
}

#[test]
fn kill_marks_dead_and_drops_from_list() {
    let (server, watch) = wired();
    let a = create(&server, "c1", 80, 24, None);
    let b = create(&server, "c2", 80, 24, None);
    let frames = server.handle_frame(&rpc("k1", methods::TERMINAL_KILL, json!({ "id": a })));
    assert_eq!(first_response(&frames).1, json!({}));

    let listed =
        first_response(&server.handle_frame(&rpc("l1", methods::TERMINAL_LIST, json!({})))).1;
    assert_eq!(listed["terminals"], json!([{ "id": b, "alive": true }]));
    assert!(!watch.is_alive(&TerminalId::from(a.as_str())));
    assert!(watch.is_alive(&TerminalId::from(b.as_str())));
}

#[test]
fn kill_then_input_is_not_found() {
    let (server, _) = wired();
    let id = create(&server, "c1", 80, 24, None);
    let _ = first_response(&server.handle_frame(&rpc(
        "k1",
        methods::TERMINAL_KILL,
        json!({ "id": id }),
    )));
    let (_, err) = first_error(&server.handle_frame(&rpc(
        "i1",
        methods::TERMINAL_INPUT,
        json!({ "id": id, "data": "x" }),
    )));
    assert_eq!(err.code, AppErrorKind::NotFound.code());
    assert_eq!(app_kind(&err), "not_found");

    let (_, err) =
        first_error(&server.handle_frame(&rpc("k2", methods::TERMINAL_KILL, json!({ "id": id }))));
    assert_eq!(err.code, AppErrorKind::NotFound.code());
}

#[test]
fn unknown_id_is_not_found() {
    let (server, _) = wired();
    for (req_id, method, params) in [
        (
            "i1",
            methods::TERMINAL_INPUT,
            json!({ "id": "term-99", "data": "x" }),
        ),
        ("k1", methods::TERMINAL_KILL, json!({ "id": "term-99" })),
    ] {
        let (id, err) = first_error(&server.handle_frame(&rpc(req_id, method, params)));
        assert_eq!(id, Id::String(req_id.into()));
        assert_eq!(err.code, AppErrorKind::NotFound.code());
        assert_eq!(app_kind(&err), "not_found");
    }
}

#[test]
fn without_hub_is_unsupported() {
    let server = Server::new();
    for (req_id, method, params) in [
        (
            "c1",
            methods::TERMINAL_CREATE,
            json!({ "cols": 80, "rows": 24 }),
        ),
        ("l1", methods::TERMINAL_LIST, json!({})),
        (
            "i1",
            methods::TERMINAL_INPUT,
            json!({ "id": "term-1", "data": "x" }),
        ),
        ("k1", methods::TERMINAL_KILL, json!({ "id": "term-1" })),
    ] {
        let (_, err) = first_error(&server.handle_frame(&rpc(req_id, method, params)));
        assert_eq!(err.code, AppErrorKind::Unsupported.code());
        assert_eq!(app_kind(&err), "unsupported");
    }
}

#[test]
fn install_on_existing_server_enables_create() {
    let server = Server::new();
    server.install_terminals(TerminalHub::new());
    assert_eq!(create(&server, "c1", 80, 24, None), "term-1");
}

#[test]
fn create_invalid_params() {
    let (server, _) = wired();
    for params in [
        json!({}),
        json!({ "rows": 24 }),
        json!({ "cols": 80 }),
        json!({ "cols": "80", "rows": 24 }),
        json!({ "cols": 80, "rows": "24" }),
        json!({ "cols": -1, "rows": 24 }),
        json!({ "cols": 80, "rows": -1 }),
        json!({ "cols": 80_000, "rows": 24 }),
        json!({ "cols": 80, "rows": 80_000 }),
        json!({ "cols": 80.5, "rows": 24 }),
        json!({ "cols": 80, "rows": 24, "cwd": "" }),
        json!({ "cols": 80, "rows": 24, "cwd": 3 }),
        json!([80, 24]),
        Value::Null,
    ] {
        let frames = server.handle_frame(&rpc("bad", methods::TERMINAL_CREATE, params.clone()));
        let (_, err) = first_error(&frames);
        assert_eq!(
            err.code,
            standard::INVALID_PARAMS,
            "params={params:?} should be invalid"
        );
    }
}

#[test]
fn input_and_kill_invalid_params() {
    let (server, _) = wired();
    let cases = [
        (methods::TERMINAL_INPUT, json!({})),
        (methods::TERMINAL_INPUT, json!({ "id": "", "data": "x" })),
        (methods::TERMINAL_INPUT, json!({ "id": 1, "data": "x" })),
        (methods::TERMINAL_INPUT, json!({ "id": "term-1" })),
        (
            methods::TERMINAL_INPUT,
            json!({ "id": "term-1", "data": 1 }),
        ),
        (methods::TERMINAL_KILL, json!({})),
        (methods::TERMINAL_KILL, json!({ "id": "" })),
        (methods::TERMINAL_KILL, json!({ "id": 1 })),
        (methods::TERMINAL_KILL, Value::Null),
        (methods::TERMINAL_INPUT, Value::Null),
    ];
    for (method, params) in cases {
        let (_, err) = first_error(&server.handle_frame(&rpc("bad", method, params.clone())));
        assert_eq!(
            err.code,
            standard::INVALID_PARAMS,
            "method={method} params={params:?} should be invalid"
        );
    }
}

#[test]
fn sessions_still_work_after_install() {
    let (server, _) = wired();
    let frames = server.handle_frame(&rpc(
        "s1",
        methods::SESSION_START,
        json!({
            "provider": "grok",
            "model": "grok-4",
            "workspace": "/ws",
        }),
    ));
    let session_id = first_response(&frames).1["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let listed =
        first_response(&server.handle_frame(&rpc("l1", methods::SESSION_LIST, json!({})))).1;
    assert_eq!(listed["sessions"][0]["id"], session_id);
}

proptest! {
    #[test]
    fn n_creates_list_length_n(n in 0usize..=12) {
        let (server, _) = wired();
        let mut ids = Vec::new();
        for i in 0..n {
            ids.push(create(&server, &format!("c{i}"), 80, 24, None));
        }
        let frames = server.handle_frame(&rpc("list", methods::TERMINAL_LIST, json!({})));
        let (_, result) = first_response(&frames);
        let terminals = result["terminals"].as_array().expect("terminals");
        prop_assert_eq!(terminals.len(), n);
        for (i, row) in terminals.iter().enumerate() {
            prop_assert_eq!(&row["id"], &ids[i]);
            prop_assert_eq!(&row["alive"], true);
        }
    }
}
