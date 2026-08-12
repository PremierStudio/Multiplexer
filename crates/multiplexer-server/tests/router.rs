//! Router contract tests. Written first (red), then the dispatcher.

use multiplexer_server::{
    BackendError, Server, SessionBackend, SessionSnapshot, SessionStartParams, SessionSummary,
    StartedSession,
};
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::error::standard;
use multiplexer_wire::error::{AppErrorKind, RpcError};
use multiplexer_wire::event::{EventKind, StreamEvent};
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use multiplexer_wire::protocol::PROTOCOL_VERSION;
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

fn rpc_num(id: i64, method: &str, params: Value) -> String {
    encode_frame(&Message::Request(Request::new(
        Id::Number(id),
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

fn event_notifications(frames: &[String]) -> Vec<StreamEvent> {
    decode_all(frames)
        .into_iter()
        .filter_map(|msg| match msg {
            Message::Notification(n) if n.method == methods::EVENT => {
                Some(serde_json::from_value(n.params).expect("event params"))
            }
            _ => None,
        })
        .collect()
}

fn start_params(provider: &str, model: &str, workspace: &str) -> Value {
    json!({
        "provider": provider,
        "model": model,
        "workspace": workspace,
    })
}

fn start_session(server: &Server, id: &str, model: &str, workspace: &str) -> String {
    let frames = server.handle_frame(&rpc(
        id,
        methods::SESSION_START,
        start_params("grok", model, workspace),
    ));
    let (resp_id, result) = first_response(&frames);
    assert_eq!(resp_id, Id::String(id.to_owned()));
    result["session_id"]
        .as_str()
        .expect("session_id string")
        .to_owned()
}

fn app_kind(err: &RpcError) -> &str {
    err.data.as_ref().map(|d| d.kind.as_str()).unwrap_or("")
}

#[test]
fn start_then_list_contains_the_session() {
    let server = Server::new();
    let session_id = start_session(&server, "s1", "grok-4", "/ws/one");

    let frames = server.handle_frame(&rpc("l1", methods::SESSION_LIST, json!({})));
    let (_, result) = first_response(&frames);
    let sessions = result["sessions"].as_array().expect("sessions array");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], session_id);
    assert_eq!(sessions[0]["model"], "grok-4");
    assert_eq!(sessions[0]["workspace"], "/ws/one");
    assert!(
        sessions[0].get("provider").is_none(),
        "list rows are id/model/workspace only"
    );
}

#[test]
fn get_unknown_is_not_found() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "g1",
        methods::SESSION_GET,
        json!({ "session_id": "sess_missing" }),
    ));
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("g1".into()));
    assert_eq!(err.code, AppErrorKind::NotFound.code());
    assert_eq!(app_kind(&err), "not_found");
}

#[test]
fn stop_then_get_is_not_found() {
    let server = Server::new();
    let session_id = start_session(&server, "s1", "grok-4", "/ws");

    let stop_frames = server.handle_frame(&rpc(
        "x1",
        methods::SESSION_STOP,
        json!({ "session_id": session_id }),
    ));
    let (_, stop_result) = first_response(&stop_frames);
    assert_eq!(stop_result, json!({}));

    let get_frames = server.handle_frame(&rpc(
        "g1",
        methods::SESSION_GET,
        json!({ "session_id": session_id }),
    ));
    let (_, err) = first_error(&get_frames);
    assert_eq!(err.code, AppErrorKind::NotFound.code());
    assert_eq!(app_kind(&err), "not_found");

    let list_frames = server.handle_frame(&rpc("l1", methods::SESSION_LIST, json!({})));
    let (_, listed) = first_response(&list_frames);
    assert_eq!(listed["sessions"], json!([]));
}

#[test]
fn turn_send_echoes_via_event_notification_frames() {
    let server = Server::new();
    let session_id = start_session(&server, "s1", "grok-4", "/ws");

    let frames = server.handle_frame(&rpc(
        "t1",
        methods::TURN_SEND,
        json!({ "session_id": session_id, "text": "hello router" }),
    ));
    let (resp_id, result) = first_response(&frames);
    assert_eq!(resp_id, Id::String("t1".into()));
    assert_eq!(result["accepted"], true);

    let events = event_notifications(&frames);
    assert!(
        !events.is_empty(),
        "turn.send must drain at least one event notification"
    );
    let echo = events
        .iter()
        .find(|e| e.event == EventKind::AgentMessageChunk && e.data["text"] == "hello router")
        .expect("echo chunk");
    assert!(echo.stream.contains(&session_id));
    assert_eq!(echo.seq, 1);
    assert_eq!(echo.in_response_to.as_ref(), Some(&Id::String("t1".into())));
}

#[test]
fn unknown_method_is_method_not_found() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("u1", "nope.nope", json!({})));
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("u1".into()));
    assert_eq!(err.code, standard::METHOD_NOT_FOUND);
    assert!(err.data.is_none(), "standard errors carry no data.kind");
}

#[test]
fn parse_error_on_bad_json() {
    let server = Server::new();
    let frames = server.handle_frame("this is not json");
    assert_eq!(frames.len(), 1);
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::Number(0));
    assert_eq!(err.code, standard::PARSE_ERROR);
    assert!(err.data.is_none());
}

proptest! {
    #[test]
    fn n_starts_list_length_n(n in 0usize..=12) {
        let server = Server::new();
        let mut ids = Vec::new();
        for i in 0..n {
            let sid = start_session(
                &server,
                &format!("s{i}"),
                &format!("model-{i}"),
                &format!("/ws/{i}"),
            );
            ids.push(sid);
        }
        let frames = server.handle_frame(&rpc("list", methods::SESSION_LIST, json!({})));
        let (_, result) = first_response(&frames);
        let sessions = result["sessions"].as_array().expect("sessions");
        prop_assert_eq!(sessions.len(), n);
        let listed: Vec<&str> = sessions
            .iter()
            .map(|s| s["id"].as_str().expect("id"))
            .collect();
        prop_assert_eq!(listed, ids.iter().map(String::as_str).collect::<Vec<_>>());
        for (i, row) in sessions.iter().enumerate() {
            prop_assert_eq!(&row["model"], &format!("model-{i}"));
            prop_assert_eq!(&row["workspace"], &format!("/ws/{i}"));
        }
    }
}

#[test]
fn list_is_empty_before_any_start() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("l0", methods::SESSION_LIST, Value::Null));
    let (_, result) = first_response(&frames);
    assert_eq!(result["sessions"], json!([]));
}

#[test]
fn get_after_start_returns_snapshot() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "s1",
        methods::SESSION_START,
        json!({
            "provider": "grok",
            "model": "grok-4",
            "workspace": "/ws",
            "initial_prompt": "hi there",
        }),
    ));
    let session_id = first_response(&frames).1["session_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let get_frames = server.handle_frame(&rpc(
        "g1",
        methods::SESSION_GET,
        json!({ "session_id": session_id }),
    ));
    let (_, snap) = first_response(&get_frames);
    assert_eq!(snap["id"], session_id);
    assert_eq!(snap["provider"], "grok");
    assert_eq!(snap["model"], "grok-4");
    assert_eq!(snap["workspace"], "/ws");
    assert_eq!(snap["initial_prompt"], "hi there");
}

#[test]
fn start_emits_session_status_event() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "s1",
        methods::SESSION_START,
        start_params("grok", "grok-4", "/ws"),
    ));
    assert!(matches!(decode_frame(&frames[0]), Ok(Message::Response(_))));
    let events = event_notifications(&frames);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event, EventKind::SessionStatus);
    assert_eq!(events[0].data["status"], "ready");
    assert_eq!(events[0].seq, 1);
    assert_eq!(
        events[0].in_response_to.as_ref(),
        Some(&Id::String("s1".into()))
    );
}

#[test]
fn second_start_does_not_replay_first_events() {
    let server = Server::new();
    let a = start_session(&server, "s1", "m-a", "/a");
    let frames = server.handle_frame(&rpc(
        "s2",
        methods::SESSION_START,
        start_params("grok", "m-b", "/b"),
    ));
    let b = first_response(&frames).1["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(a, b);
    let events = event_notifications(&frames);
    assert!(events.iter().all(|e| e.stream.contains(&b)));
    assert!(events.iter().all(|e| !e.stream.contains(&a)));
}

#[test]
fn missing_start_params_are_invalid() {
    let server = Server::new();
    for params in [
        json!({}),
        json!({ "model": "m", "workspace": "/w" }),
        json!({ "provider": "p", "workspace": "/w" }),
        json!({ "provider": "p", "model": "m" }),
        json!({ "provider": "", "model": "m", "workspace": "/w" }),
        json!({ "provider": "p", "model": "", "workspace": "/w" }),
        json!({ "provider": "p", "model": "m", "workspace": "" }),
        json!({ "provider": 1, "model": "m", "workspace": "/w" }),
        json!({ "provider": "p", "model": "m", "workspace": "/w", "initial_prompt": 3 }),
        json!(["grok", "m", "/w"]),
        Value::Null,
    ] {
        let frames = server.handle_frame(&rpc("bad", methods::SESSION_START, params.clone()));
        let (_, err) = first_error(&frames);
        assert_eq!(
            err.code,
            standard::INVALID_PARAMS,
            "params={params:?} should be invalid"
        );
    }
}

#[test]
fn missing_get_stop_turn_params_are_invalid() {
    let server = Server::new();
    let cases = [
        (methods::SESSION_GET, json!({})),
        (methods::SESSION_GET, json!({ "session_id": "" })),
        (methods::SESSION_GET, json!({ "session_id": 1 })),
        (methods::SESSION_STOP, json!({})),
        (methods::SESSION_STOP, json!({ "session_id": "" })),
        (methods::SESSION_INTERRUPT, json!({})),
        (methods::SESSION_INTERRUPT, json!({ "session_id": "" })),
        (methods::SESSION_INTERRUPT, json!({ "session_id": 1 })),
        (methods::TURN_SEND, json!({ "text": "hi" })),
        (methods::TURN_SEND, json!({ "session_id": "sess_1" })),
        (
            methods::TURN_SEND,
            json!({ "session_id": "", "text": "hi" }),
        ),
        (
            methods::TURN_SEND,
            json!({ "session_id": "sess_1", "text": 9 }),
        ),
        (methods::TURN_SEND, Value::Null),
        (methods::APPROVAL_RESPOND, json!({})),
        (
            methods::APPROVAL_RESPOND,
            json!({ "session_id": "s", "request_id": "r" }),
        ),
        (
            methods::APPROVAL_RESPOND,
            json!({ "session_id": "s", "decision": "allow" }),
        ),
        (
            methods::APPROVAL_RESPOND,
            json!({ "request_id": "r", "decision": "allow" }),
        ),
        (
            methods::APPROVAL_RESPOND,
            json!({ "session_id": "", "request_id": "r", "decision": "allow" }),
        ),
        (
            methods::APPROVAL_RESPOND,
            json!({ "session_id": "s", "request_id": "", "decision": "allow" }),
        ),
        (
            methods::APPROVAL_RESPOND,
            json!({ "session_id": "s", "request_id": "r", "decision": "" }),
        ),
        (
            methods::APPROVAL_RESPOND,
            json!({ "session_id": "s", "request_id": "r", "decision": 1 }),
        ),
        (methods::GIT_WORKTREES, json!({})),
        (methods::GIT_WORKTREES, json!({ "cwd": "" })),
        (methods::GIT_WORKTREES, json!({ "cwd": 1 })),
        (methods::GIT_WORKTREES, Value::Null),
    ];
    for (method, params) in cases {
        let frames = server.handle_frame(&rpc("bad", method, params.clone()));
        let (_, err) = first_error(&frames);
        assert_eq!(err.code, standard::INVALID_PARAMS, "{method} {params}");
    }
}

#[test]
fn turn_send_unknown_session_is_not_found() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "t1",
        methods::TURN_SEND,
        json!({ "session_id": "sess_nope", "text": "hi" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(app_kind(&err), "not_found");
}

#[test]
fn stop_unknown_session_is_not_found() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "x1",
        methods::SESSION_STOP,
        json!({ "session_id": "sess_nope" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(app_kind(&err), "not_found");
}

#[test]
fn numeric_id_is_echoed() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc_num(7, methods::SYSTEM_PING, json!({})));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::Number(7));
    assert_eq!(result["pong"], true);
}

#[test]
fn system_hello_returns_protocol_version() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "h1",
        methods::SYSTEM_HELLO,
        json!({ "protocol_version": PROTOCOL_VERSION }),
    ));
    let (_, result) = first_response(&frames);
    assert_eq!(result["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(result["server_info"]["name"], "multiplexer-server");
}

#[test]
fn system_hello_rejects_version_mismatch() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "h1",
        methods::SYSTEM_HELLO,
        json!({ "protocol_version": "99.0.0" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, AppErrorKind::ProtocolVersionMismatch.code());
    assert_eq!(app_kind(&err), "protocol_version_mismatch");
}

#[test]
fn invalid_request_echoes_id_when_present() {
    let server = Server::new();
    let frames = server.handle_frame(r#"{"jsonrpc":"2.0","id":"abc","method":1}"#);
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("abc".into()));
    assert_eq!(err.code, standard::INVALID_REQUEST);
}

#[test]
fn invalid_request_without_id_uses_zero() {
    let server = Server::new();
    let frames = server.handle_frame(r#"{"jsonrpc":"2.0"}"#);
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::Number(0));
    assert_eq!(err.code, standard::INVALID_REQUEST);
}

#[test]
fn inbound_notification_has_no_response() {
    let server = Server::new();
    let frames = server.handle_frame(r#"{"jsonrpc":"2.0","method":"system.ping","params":{}}"#);
    assert!(frames.is_empty());
}

#[test]
fn inbound_response_has_no_reply() {
    let server = Server::new();
    let frames = server.handle_frame(r#"{"jsonrpc":"2.0","id":"x","result":true}"#);
    assert!(frames.is_empty());
}

#[test]
fn inbound_error_has_no_reply() {
    let server = Server::new();
    let frames =
        server.handle_frame(r#"{"jsonrpc":"2.0","id":"x","error":{"code":-32600,"message":"no"}}"#);
    assert!(frames.is_empty());
}

#[test]
fn turn_send_empty_text_echoes() {
    let server = Server::new();
    let session_id = start_session(&server, "s1", "m", "/w");
    let frames = server.handle_frame(&rpc(
        "t1",
        methods::TURN_SEND,
        json!({ "session_id": session_id, "text": "" }),
    ));
    let echo = event_notifications(&frames)
        .into_iter()
        .find(|e| e.event == EventKind::AgentMessageChunk)
        .expect("echo");
    assert_eq!(echo.data["text"], "");
}

#[test]
fn two_turns_increment_seq() {
    let server = Server::new();
    let session_id = start_session(&server, "s1", "m", "/w");
    let first = server.handle_frame(&rpc(
        "t1",
        methods::TURN_SEND,
        json!({ "session_id": session_id, "text": "one" }),
    ));
    let second = server.handle_frame(&rpc(
        "t2",
        methods::TURN_SEND,
        json!({ "session_id": session_id, "text": "two" }),
    ));
    let e1 = event_notifications(&first)
        .into_iter()
        .find(|e| e.event == EventKind::AgentMessageChunk)
        .unwrap();
    let e2 = event_notifications(&second)
        .into_iter()
        .find(|e| e.event == EventKind::AgentMessageChunk)
        .unwrap();
    assert_eq!(e1.seq, 1);
    assert_eq!(e2.seq, 2);
    assert_eq!(e2.data["text"], "two");
}

#[test]
fn default_server_handles_ping() {
    let server = Server::default();
    let frames = server.handle_frame(&rpc("p", methods::SYSTEM_PING, json!({})));
    assert_eq!(first_response(&frames).1["pong"], true);
}

#[test]
fn stop_first_of_two_leaves_the_second() {
    let server = Server::new();
    let a = start_session(&server, "s1", "m-a", "/a");
    let b = start_session(&server, "s2", "m-b", "/b");
    let _ = server.handle_frame(&rpc("x", methods::SESSION_STOP, json!({ "session_id": a })));
    let frames = server.handle_frame(&rpc("l", methods::SESSION_LIST, json!({})));
    let sessions = first_response(&frames).1["sessions"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], b);
    assert_eq!(sessions[0]["model"], "m-b");
}

#[test]
fn system_hello_without_version_succeeds() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("h", methods::SYSTEM_HELLO, json!({})));
    let (_, result) = first_response(&frames);
    assert_eq!(result["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(result["server_info"]["version"], "0.1.0");
}

#[test]
fn system_hello_non_string_version_is_invalid_params() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "h",
        methods::SYSTEM_HELLO,
        json!({ "protocol_version": 1 }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, standard::INVALID_PARAMS);
}

#[test]
fn invalid_request_echoes_numeric_id() {
    let server = Server::new();
    let frames = server.handle_frame(r#"{"jsonrpc":"2.0","id":42,"method":1}"#);
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::Number(42));
    assert_eq!(err.code, standard::INVALID_REQUEST);
}

#[test]
fn non_integer_id_falls_back_to_zero() {
    let server = Server::new();
    let frames = server.handle_frame(r#"{"jsonrpc":"2.0","id":1.5,"method":1}"#);
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::Number(0));
    assert_eq!(err.code, standard::INVALID_REQUEST);
}

#[test]
fn null_initial_prompt_is_omitted_from_snapshot() {
    let server = Server::new();
    let start = server.handle_frame(&rpc(
        "s1",
        methods::SESSION_START,
        json!({
            "provider": "grok",
            "model": "m",
            "workspace": "/w",
            "initial_prompt": null,
        }),
    ));
    let session_id = first_response(&start).1["session_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let get = server.handle_frame(&rpc(
        "g1",
        methods::SESSION_GET,
        json!({ "session_id": session_id }),
    ));
    let snap = first_response(&get).1;
    assert!(snap.get("initial_prompt").is_none());
}

struct RejectAll;

impl SessionBackend for RejectAll {
    fn start(&mut self, _: SessionStartParams) -> Result<StartedSession, BackendError> {
        Err(BackendError::NotFound {
            session_id: "new".into(),
        })
    }

    fn list(&self) -> Vec<SessionSummary> {
        Vec::new()
    }

    fn get(&self, session_id: &str) -> Result<SessionSnapshot, BackendError> {
        Err(BackendError::NotFound {
            session_id: session_id.to_owned(),
        })
    }

    fn stop(&mut self, session_id: &str) -> Result<(), BackendError> {
        Err(BackendError::NotFound {
            session_id: session_id.to_owned(),
        })
    }

    fn send_turn(&mut self, session_id: &str, _: &str) -> Result<(), BackendError> {
        Err(BackendError::NotFound {
            session_id: session_id.to_owned(),
        })
    }

    fn interrupt(&mut self, session_id: &str) -> Result<(), BackendError> {
        Err(BackendError::NotFound {
            session_id: session_id.to_owned(),
        })
    }

    fn approval_respond(
        &mut self,
        session_id: &str,
        _: &str,
        _: multiplexer_wire::approval::ApprovalDecision,
    ) -> Result<(), BackendError> {
        Err(BackendError::NotFound {
            session_id: session_id.to_owned(),
        })
    }

    fn drain_events(&mut self) -> Vec<StreamEvent> {
        Vec::new()
    }
}

#[test]
fn start_backend_error_is_not_found() {
    let server = Server::with_backend(RejectAll);
    let frames = server.handle_frame(&rpc(
        "s",
        methods::SESSION_START,
        start_params("p", "m", "/w"),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(app_kind(&err), "not_found");
    assert!(err.message.contains("new"));
}

#[test]
fn interrupt_unknown_is_not_found() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "i1",
        methods::SESSION_INTERRUPT,
        json!({ "session_id": "sess_missing" }),
    ));
    let (id, err) = first_error(&frames);
    assert_eq!(id, Id::String("i1".into()));
    assert_eq!(err.code, AppErrorKind::NotFound.code());
    assert_eq!(app_kind(&err), "not_found");
}

#[test]
fn interrupt_known_session_is_ok() {
    let server = Server::new();
    let session_id = start_session(&server, "s1", "grok-4", "/ws");
    let frames = server.handle_frame(&rpc(
        "i1",
        methods::SESSION_INTERRUPT,
        json!({ "session_id": session_id }),
    ));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("i1".into()));
    assert_eq!(result, json!({}));
}

#[test]
fn approval_respond_invalid_decision_is_invalid_params() {
    let server = Server::new();
    for decision in ["maybe", "yes", "ALLOW", "allow-once", "true"] {
        let frames = server.handle_frame(&rpc(
            "a1",
            methods::APPROVAL_RESPOND,
            json!({
                "session_id": "sess_1",
                "request_id": "req_1",
                "decision": decision,
            }),
        ));
        let (id, err) = first_error(&frames);
        assert_eq!(id, Id::String("a1".into()));
        assert_eq!(
            err.code,
            standard::INVALID_PARAMS,
            "decision={decision} should be invalid"
        );
    }
}

#[test]
fn git_worktrees_returns_parsed_list_from_fake_git() {
    let git = multiplexer_worktree::FakeGit::new();
    git.push(Ok(
        "worktree /repo\nHEAD abc123\nbranch refs/heads/main\nlocked\n".into(),
    ));
    let server = Server::with_git(multiplexer_worktree::WorktreeService::new(git));
    let frames = server.handle_frame(&rpc(
        "g1",
        methods::GIT_WORKTREES,
        json!({ "cwd": "/repo" }),
    ));
    let (id, result) = first_response(&frames);
    assert_eq!(id, Id::String("g1".into()));
    let trees = result["worktrees"].as_array().expect("worktrees array");
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0]["path"], "/repo");
    assert_eq!(trees[0]["head"], "abc123");
    assert_eq!(trees[0]["branch"], "refs/heads/main");
    assert_eq!(trees[0]["detached"], false);
    assert_eq!(trees[0]["locked"], true);
    assert_eq!(trees[0]["prunable"], false);
}

#[test]
fn git_worktrees_server_still_handles_sessions() {
    let git = multiplexer_worktree::FakeGit::new();
    let server = Server::with_git(multiplexer_worktree::WorktreeService::new(git));
    let session_id = start_session(&server, "s1", "grok-4", "/ws");
    let frames = server.handle_frame(&rpc("l1", methods::SESSION_LIST, json!({})));
    let listed = first_response(&frames).1;
    let sessions = listed["sessions"].as_array().expect("sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["id"], session_id);
}

#[test]
fn git_worktrees_without_catalog_is_unsupported() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc(
        "g1",
        methods::GIT_WORKTREES,
        json!({ "cwd": "/repo" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, AppErrorKind::Unsupported.code());
    assert_eq!(app_kind(&err), "unsupported");
}

#[test]
fn system_hello_null_params_succeeds() {
    let server = Server::new();
    let frames = server.handle_frame(&rpc("h", methods::SYSTEM_HELLO, Value::Null));
    let (_, result) = first_response(&frames);
    assert_eq!(result["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(result["server_info"]["name"], "multiplexer-server");
}

#[test]
fn git_worktrees_catalog_error_is_provider_error() {
    let git = multiplexer_worktree::FakeGit::new();
    let server = Server::with_git(multiplexer_worktree::WorktreeService::new(git));
    let frames = server.handle_frame(&rpc(
        "g1",
        methods::GIT_WORKTREES,
        json!({ "cwd": "/repo" }),
    ));
    let (_, err) = first_error(&frames);
    assert_eq!(err.code, AppErrorKind::ProviderError.code());
    assert_eq!(app_kind(&err), "provider_error");
}

#[test]
fn install_git_on_existing_server_enables_worktrees() {
    let server = Server::new();
    let git = multiplexer_worktree::FakeGit::new();
    git.push(Ok(
        "worktree /repo\nHEAD abc\nbranch refs/heads/main\n".into()
    ));
    server.install_git(multiplexer_worktree::WorktreeService::new(git));
    let frames = server.handle_frame(&rpc(
        "g1",
        methods::GIT_WORKTREES,
        json!({ "cwd": "/repo" }),
    ));
    let result = first_response(&frames).1;
    let trees = result["worktrees"].as_array().expect("worktrees");
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0]["path"], "/repo");
}
