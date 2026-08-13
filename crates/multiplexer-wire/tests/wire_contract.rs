//! Contract-level tests for the JSON-RPC 2.0 wire envelope (plan/04).
//!
//! TDD: written FIRST (red), implementation added to make them pass (green).
//! These pin the wire format to the spec in plan/04 §3 (framing), §7 (error
//! model) and §5 (event/notification types): version field, id handling,
//! request/response/error/notification shapes, the discriminated `event`
//! notification, and the stable machine-readable `data.kind` error codes.

use multiplexer_wire::codec::{decode_frame, encode_frame, CodecError};
use multiplexer_wire::error::standard;
use multiplexer_wire::error::{AppErrorKind, RpcError};
use multiplexer_wire::event::{EventKind, StreamEvent};
use multiplexer_wire::jsonrpc::{ErrorResponse, Id, Message, Notification, Request, Response};
use multiplexer_wire::methods;
use multiplexer_wire::protocol::PROTOCOL_VERSION;
use serde_json::json;

// ---------------------------------------------------------------------------
// 1. Request framing (plan/04 §3.1)
// ---------------------------------------------------------------------------

#[test]
fn request_serializes_to_spec_shape() {
    let req = Request::new(
        Id::String("req_01".into()),
        "turn.send",
        json!({ "thread_id": "thr_01", "text": "refactor" }),
    );
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], "req_01");
    assert_eq!(json["method"], "turn.send");
    assert_eq!(json["params"]["thread_id"], "thr_01");
    assert_eq!(json["params"]["text"], "refactor");
}

#[test]
fn request_round_trips_through_codec() {
    let req = Request::new(
        Id::String("req_01".into()),
        "turn.send",
        json!({ "thread_id": "thr_01" }),
    );
    let wire = encode_frame(&Message::Request(req.clone())).unwrap();
    let back = decode_frame(&wire).unwrap();
    assert_eq!(back, Message::Request(req));
}

#[test]
fn request_supports_numeric_id() {
    let req = Request::new(Id::Number(7), "system.ping", json!({}));
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["id"], 7);
    let wire = encode_frame(&Message::Request(req)).unwrap();
    let back = decode_frame(&wire).unwrap();
    assert!(matches!(back, Message::Request(r) if r.id == Id::Number(7)));
}

// ---------------------------------------------------------------------------
// 2. Response framing (plan/04 §3.2)
// ---------------------------------------------------------------------------

#[test]
fn response_serializes_to_spec_shape() {
    let resp = Response::new(
        Id::String("req_01".into()),
        json!({ "turn_id": "trn_01", "accepted": true }),
    );
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], "req_01");
    assert_eq!(json["result"]["turn_id"], "trn_01");
    assert_eq!(json["result"]["accepted"], true);
}

#[test]
fn response_round_trips_through_codec() {
    let resp = Response::new(Id::Number(3), json!({ "pong": true }));
    let wire = encode_frame(&Message::Response(resp.clone())).unwrap();
    let back = decode_frame(&wire).unwrap();
    assert_eq!(back, Message::Response(resp));
}

// ---------------------------------------------------------------------------
// 3. Error response framing (plan/04 §3.3, §7)
// ---------------------------------------------------------------------------

#[test]
fn app_error_serializes_with_code_and_data_kind() {
    let err = RpcError::app(
        AppErrorKind::Conflict,
        "a turn is already running on this thread",
    );
    let resp = ErrorResponse::new(Id::String("req_01".into()), err);
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["id"], "req_01");
    assert_eq!(json["error"]["code"], -32005);
    assert_eq!(
        json["error"]["message"],
        "a turn is already running on this thread"
    );
    assert_eq!(json["error"]["data"]["kind"], "conflict");
}

#[test]
fn app_error_with_details_serializes_details() {
    let err = RpcError::app_with_details(
        AppErrorKind::PathInvalid,
        "path outside worktree",
        Some(json!({ "field": "path", "reason": "traversal" })),
    );
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["code"], -32007);
    assert_eq!(json["data"]["kind"], "path_invalid");
    assert_eq!(json["data"]["details"]["field"], "path");
}

#[test]
fn standard_error_has_no_data_kind() {
    let err = RpcError::new(-32601, "method not found");
    let json = serde_json::to_value(&err).unwrap();
    assert_eq!(json["code"], -32601);
    assert_eq!(json["message"], "method not found");
    assert!(
        json.get("data").is_none(),
        "standard errors carry no data.kind"
    );
}

#[test]
fn error_response_round_trips_through_codec() {
    let err = RpcError::app(AppErrorKind::NotFound, "thread missing");
    let resp = ErrorResponse::new(Id::String("req_01".into()), err);
    let wire = encode_frame(&Message::Error(resp.clone())).unwrap();
    let back = decode_frame(&wire).unwrap();
    assert_eq!(back, Message::Error(resp));
}

// ---------------------------------------------------------------------------
// 4. Notification framing (plan/04 §3.4) and event notification (§3.5, §5)
// ---------------------------------------------------------------------------

#[test]
fn notification_has_no_id() {
    let notif = Notification::new(
        "terminal.input",
        json!({ "pty_id": "pty_01", "data": "ls\r" }),
    );
    let json = serde_json::to_value(&notif).unwrap();
    assert_eq!(json["jsonrpc"], "2.0");
    assert_eq!(json["method"], "terminal.input");
    assert_eq!(json["params"]["pty_id"], "pty_01");
    assert!(
        json.get("id").is_none(),
        "notifications must not carry an id"
    );
}

#[test]
fn notification_round_trips_through_codec() {
    let notif = Notification::new(
        "userInput.respond",
        json!({ "request_id": "r1", "text": "ok" }),
    );
    let wire = encode_frame(&Message::Notification(notif.clone())).unwrap();
    let back = decode_frame(&wire).unwrap();
    assert_eq!(back, Message::Notification(notif));
}

#[test]
fn event_notification_uses_single_event_method_with_discriminated_field() {
    let ev = StreamEvent::new(
        "turn:thr_01".into(),
        EventKind::AgentMessageChunk,
        42,
        json!({ "text": "Refactoring..." }),
    );
    let notif = ev.to_notification();
    let json = serde_json::to_value(&notif).unwrap();
    assert_eq!(json["method"], "event");
    assert_eq!(json["params"]["stream"], "turn:thr_01");
    assert_eq!(json["params"]["event"], "agent_message_chunk");
    assert_eq!(json["params"]["seq"], 42);
    assert_eq!(json["params"]["data"]["text"], "Refactoring...");
}

#[test]
fn event_notification_carries_in_response_to_when_set() {
    let mut ev = StreamEvent::new(
        "turn:thr_01".into(),
        EventKind::AgentMessageChunk,
        1,
        json!({ "text": "hi" }),
    );
    ev.in_response_to = Some(Id::String("req_01".into()));
    let json = serde_json::to_value(ev.to_notification()).unwrap();
    assert_eq!(json["params"]["in_response_to"], "req_01");
}

#[test]
fn event_notification_omits_in_response_to_when_unset() {
    let ev = StreamEvent::new(
        "turn:thr_01".into(),
        EventKind::AgentMessageChunk,
        1,
        json!({ "text": "hi" }),
    );
    let json = serde_json::to_value(ev.to_notification()).unwrap();
    assert!(
        json["params"].get("in_response_to").is_none(),
        "absent in_response_to must not serialize"
    );
}

#[test]
fn event_round_trips_through_codec() {
    let ev = StreamEvent::new(
        "terminal:pty_01".into(),
        EventKind::TerminalOutput,
        7,
        json!({ "data": "bHMgLXI=", "encoding": "base64" }),
    );
    let wire = encode_frame(&Message::Notification(ev.to_notification())).unwrap();
    let back = decode_frame(&wire).unwrap();
    match back {
        Message::Notification(n) => {
            let parsed: StreamEvent = serde_json::from_value(n.params).unwrap();
            assert_eq!(parsed.stream, "terminal:pty_01");
            assert_eq!(parsed.event, EventKind::TerminalOutput);
            assert_eq!(parsed.seq, 7);
        }
        other => panic!("expected notification, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 5. Version field (plan/04 §3, §9.2)
// ---------------------------------------------------------------------------

#[test]
fn protocol_version_is_a_semver_string() {
    // plan/04 §9.2: protocol_version is a semver string (e.g. 1.2.0).
    assert_eq!(PROTOCOL_VERSION, "0.1.0");
    let parts: Vec<&str> = PROTOCOL_VERSION.split('.').collect();
    assert_eq!(parts.len(), 3, "must be major.minor.patch");
    for p in &parts {
        assert!(
            !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()),
            "bad part {p}"
        );
    }
}

#[test]
fn decode_rejects_missing_jsonrpc_version() {
    let err = decode_frame(r#"{"id":1,"method":"system.ping","params":{}}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_rejects_wrong_jsonrpc_version() {
    let err =
        decode_frame(r#"{"jsonrpc":"1.0","id":1,"method":"system.ping","params":{}}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_rejects_non_object_frame() {
    for bad in ["[1,2,3]", "\"hello\"", "42", "null"] {
        let err = decode_frame(bad).unwrap_err();
        assert!(
            matches!(err, CodecError::InvalidRequest(_)),
            "frame {bad} must be InvalidRequest"
        );
    }
}

#[test]
fn decode_rejects_malformed_json() {
    let err = decode_frame("{ not json").unwrap_err();
    assert!(matches!(err, CodecError::Parse(_)));
}

// ---------------------------------------------------------------------------
// 6. Id handling (plan/04 §3.1)
// ---------------------------------------------------------------------------

#[test]
fn decode_rejects_non_integer_or_string_ids() {
    // JSON-RPC ids must be strings or integers; floats, objects, and arrays
    // are rejected (plan/04 §3.1).
    for bad in [
        r#"{"jsonrpc":"2.0","id":1.5,"method":"x","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":{},"method":"x","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":[1],"method":"x","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":true,"method":"x","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":null,"method":"x","params":{}}"#,
    ] {
        let err = decode_frame(bad).unwrap_err();
        assert!(
            matches!(err, CodecError::InvalidRequest(_)),
            "frame {bad} must be InvalidRequest"
        );
    }
}

#[test]
fn decode_rejects_malformed_error_object() {
    // The error object must be well-formed (integer code, string message);
    // garbage there is an invalid request, not a crash.
    let err = decode_frame(r#"{"jsonrpc":"2.0","id":1,"error":{"code":"nope"}}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_rejects_frame_with_neither_method_nor_result_nor_error() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","id":1}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn message_id_accessor_reflects_each_kind() {
    let req = Request::new(Id::String("req_01".into()), "turn.send", json!({}));
    assert_eq!(
        Message::Request(req).id(),
        Some(&Id::String("req_01".into()))
    );

    let resp = Response::new(Id::Number(5), json!({}));
    assert_eq!(Message::Response(resp).id(), Some(&Id::Number(5)));

    let err = RpcError::app(AppErrorKind::NotFound, "missing");
    let e = ErrorResponse::new(Id::String("req_09".into()), err);
    assert_eq!(Message::Error(e).id(), Some(&Id::String("req_09".into())));

    let notif = Notification::new("event", json!({}));
    assert_eq!(Message::Notification(notif).id(), None);
}

#[test]
fn decode_rejects_request_without_id() {
    // A message with method but no id is a notification, not a request.
    let msg = decode_frame(r#"{"jsonrpc":"2.0","method":"turn.send","params":{}}"#).unwrap();
    assert!(
        matches!(msg, Message::Notification(_)),
        "method without id must decode as a notification"
    );
}

#[test]
fn decode_rejects_response_without_id() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","result":{}}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_rejects_ambiguous_frame_with_id_and_result_and_error() {
    let err =
        decode_frame(r#"{"jsonrpc":"2.0","id":1,"result":{},"error":{"code":-1,"message":"x"}}"#)
            .unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_rejects_frame_with_id_and_method_and_result() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","id":1,"method":"x","result":{}}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_rejects_frame_with_method_and_error() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","method":"x","error":{"code":-1,"message":"x"}}"#)
        .unwrap_err();
    assert!(
        matches!(err, CodecError::InvalidRequest(s) if s.contains("cannot carry result or error"))
    );
}

#[test]
fn decode_rejects_non_string_method() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","method":1,"params":{}}"#).unwrap_err();
    assert!(matches!(
        err,
        CodecError::InvalidRequest(s) if s.contains("method must be a string")
    ));
}

#[test]
fn decode_rejects_non_string_jsonrpc_version() {
    let err =
        decode_frame(r#"{"jsonrpc":2.0,"id":1,"method":"system.ping","params":{}}"#).unwrap_err();
    assert!(matches!(err, CodecError::InvalidRequest(_)));
}

#[test]
fn decode_missing_params_defaults_to_null() {
    let msg = decode_frame(r#"{"jsonrpc":"2.0","id":1,"method":"system.ping"}"#).unwrap();
    match msg {
        Message::Request(r) => assert_eq!(r.params, serde_json::Value::Null),
        other => panic!("expected request, got {other:?}"),
    }
}

#[test]
fn decode_notification_missing_params_defaults_to_null() {
    let msg = decode_frame(r#"{"jsonrpc":"2.0","method":"event"}"#).unwrap();
    match msg {
        Message::Notification(n) => {
            assert_eq!(n.method, "event");
            assert_eq!(n.params, serde_json::Value::Null);
        }
        other => panic!("expected notification, got {other:?}"),
    }
}

#[test]
fn decode_rejects_invalid_id_on_success_response() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","id":true,"result":{}}"#).unwrap_err();
    assert!(matches!(
        err,
        CodecError::InvalidRequest(s) if s.contains("id must be a string or integer")
    ));
}

#[test]
fn decode_rejects_invalid_id_on_error_response() {
    let err = decode_frame(r#"{"jsonrpc":"2.0","id":null,"error":{"code":-1,"message":"x"}}"#)
        .unwrap_err();
    assert!(matches!(
        err,
        CodecError::InvalidRequest(s) if s.contains("id must be a string or integer")
    ));
}

// ---------------------------------------------------------------------------
// 7. App error kind codes (plan/04 §7.2)
// ---------------------------------------------------------------------------

#[test]
fn standard_error_codes_match_spec() {
    assert_eq!(standard::PARSE_ERROR, -32700);
    assert_eq!(standard::INVALID_REQUEST, -32600);
    assert_eq!(standard::METHOD_NOT_FOUND, -32601);
    assert_eq!(standard::INVALID_PARAMS, -32602);
    assert_eq!(standard::INTERNAL_ERROR, -32603);
}

#[test]
fn app_error_kind_codes_match_spec() {
    let codes = [
        (AppErrorKind::AuthRequired, -32000),
        (AppErrorKind::AuthExpired, -32001),
        (AppErrorKind::TicketInvalid, -32002),
        (AppErrorKind::PermissionDenied, -32003),
        (AppErrorKind::NotFound, -32004),
        (AppErrorKind::Conflict, -32005),
        (AppErrorKind::InvalidState, -32006),
        (AppErrorKind::PathInvalid, -32007),
        (AppErrorKind::ProviderError, -32008),
        (AppErrorKind::RateLimited, -32009),
        (AppErrorKind::Unsupported, -32010),
        (AppErrorKind::StreamClosed, -32011),
        (AppErrorKind::ProtocolVersionMismatch, -32012),
    ];
    for (kind, want) in codes {
        assert_eq!(kind.code(), want, "{kind:?}");
        assert!(kind.code() < 0, "{kind:?} code must be negative");
        assert_ne!(kind.code(), want.unsigned_abs() as i64, "{kind:?}");
    }
}

#[test]
fn app_error_kind_serializes_to_snake_case_kind() {
    for (kind, expected) in [
        (AppErrorKind::AuthRequired, "auth_required"),
        (AppErrorKind::AuthExpired, "auth_expired"),
        (AppErrorKind::TicketInvalid, "ticket_invalid"),
        (AppErrorKind::PermissionDenied, "permission_denied"),
        (AppErrorKind::NotFound, "not_found"),
        (AppErrorKind::Conflict, "conflict"),
        (AppErrorKind::InvalidState, "invalid_state"),
        (AppErrorKind::PathInvalid, "path_invalid"),
        (AppErrorKind::ProviderError, "provider_error"),
        (AppErrorKind::RateLimited, "rate_limited"),
        (AppErrorKind::Unsupported, "unsupported"),
        (AppErrorKind::StreamClosed, "stream_closed"),
        (
            AppErrorKind::ProtocolVersionMismatch,
            "protocol_version_mismatch",
        ),
    ] {
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{expected}\""),
            "kind {kind:?}"
        );
    }
}

#[test]
fn app_error_kind_deserializes_from_snake_case() {
    assert_eq!(
        serde_json::from_str::<AppErrorKind>("\"protocol_version_mismatch\"").unwrap(),
        AppErrorKind::ProtocolVersionMismatch
    );
    assert_eq!(
        serde_json::from_str::<AppErrorKind>("\"permission_denied\"").unwrap(),
        AppErrorKind::PermissionDenied
    );
}

// ---------------------------------------------------------------------------
// 8. Method-name constants (plan/04 §4)
// ---------------------------------------------------------------------------

#[test]
fn method_constants_match_spec_namespaces() {
    let expected = [
        (methods::EVENT, "event"),
        (methods::SESSION_START, "session.start"),
        (methods::SESSION_STOP, "session.stop"),
        (methods::SESSION_INTERRUPT, "session.interrupt"),
        (methods::SESSION_LIST, "session.list"),
        (methods::SESSION_GET, "session.get"),
        (methods::TURN_SEND, "turn.send"),
        (methods::TURN_CANCEL, "turn.cancel"),
        (methods::TURN_HISTORY, "turn.history"),
        (methods::APPROVAL_RESPOND, "approval.respond"),
        (methods::APPROVAL_LIST, "approval.list"),
        (methods::USER_INPUT_RESPOND, "userInput.respond"),
        (methods::USER_INPUT_CANCEL, "userInput.cancel"),
        (methods::CHECKPOINT_LIST, "checkpoint.list"),
        (methods::CHECKPOINT_CREATE, "checkpoint.create"),
        (methods::CHECKPOINT_DIFF, "checkpoint.diff"),
        (methods::CHECKPOINT_REVERT, "checkpoint.revert"),
        (methods::CHECKPOINT_APPLY, "checkpoint.apply"),
        (methods::TERMINAL_CREATE, "terminal.create"),
        (methods::TERMINAL_RESIZE, "terminal.resize"),
        (methods::TERMINAL_INPUT, "terminal.input"),
        (methods::TERMINAL_KILL, "terminal.kill"),
        (methods::TERMINAL_LIST, "terminal.list"),
        (methods::TERMINAL_ATTACH, "terminal.attach"),
        (methods::FS_READ, "fs.read"),
        (methods::FS_WRITE, "fs.write"),
        (methods::FS_LIST, "fs.list"),
        (methods::FS_WATCH, "fs.watch"),
        (methods::FS_UNWATCH, "fs.unwatch"),
        (methods::FS_STAT, "fs.stat"),
        (methods::GIT_STATUS, "git.status"),
        (methods::GIT_DIFF, "git.diff"),
        (methods::GIT_COMMIT, "git.commit"),
        (methods::GIT_BRANCHES, "git.branches"),
        (methods::GIT_CHECKOUT, "git.checkout"),
        (methods::GIT_WORKTREES, "git.worktrees"),
        (methods::GIT_WORKTREE_CREATE, "git.worktree.create"),
        (methods::BROWSER_LIST, "browser.list"),
        (methods::BROWSER_LAUNCH, "browser.launch"),
        (methods::BROWSER_NAVIGATE, "browser.navigate"),
        (methods::BROWSER_CDP, "browser.cdp"),
        (methods::BROWSER_CLOSE, "browser.close"),
        (methods::BROWSER_SCREENSHOT, "browser.screenshot"),
        (methods::HAR_START, "har.start"),
        (methods::HAR_STOP, "har.stop"),
        (methods::HAR_REPLAY, "har.replay"),
        (methods::HAR_LIST, "har.list"),
        (methods::ORCHESTRATION_SPAWN, "orchestration.spawn"),
        (methods::ORCHESTRATION_SUBSCRIBE, "orchestration.subscribe"),
        (
            methods::ORCHESTRATION_UNSUBSCRIBE,
            "orchestration.unsubscribe",
        ),
        (methods::ORCHESTRATION_LIST, "orchestration.list"),
        (methods::MODEL_LIST, "model.list"),
        (methods::MODEL_SELECT, "model.select"),
        (methods::MODEL_GET, "model.get"),
        (methods::REMOTE_LIST, "remote.list"),
        (methods::REMOTE_CONNECT, "remote.connect"),
        (methods::REMOTE_DISCONNECT, "remote.disconnect"),
        (methods::AUTH_PROVIDERS, "auth.providers"),
        (methods::AUTH_LOGIN, "auth.login"),
        (methods::AUTH_STATUS, "auth.status"),
        (methods::AUTH_LOGOUT, "auth.logout"),
        (methods::TELEMETRY_USAGE, "telemetry.usage"),
        (methods::TELEMETRY_RESOURCES, "telemetry.resources"),
        (methods::TELEMETRY_SUBSCRIBE, "telemetry.subscribe"),
        (methods::SYSTEM_HELLO, "system.hello"),
        (methods::SYSTEM_PING, "system.ping"),
        (methods::SYSTEM_CAPABILITIES, "system.capabilities"),
        (methods::SUBSCRIBE, "subscribe"),
        (methods::UNSUBSCRIBE, "unsubscribe"),
        (methods::ATTACH_STREAM, "attach_stream"),
        (methods::STREAM_ACK, "stream.ack"),
    ];
    for (got, want) in expected {
        assert_eq!(got, want);
    }
}

// ---------------------------------------------------------------------------
// 9. Codec error display
// ---------------------------------------------------------------------------

#[test]
fn codec_errors_are_displayable() {
    let e = decode_frame("{ not json").unwrap_err();
    assert!(e.to_string().starts_with("parse error:"));
    let e = decode_frame("[]").unwrap_err();
    assert!(e.to_string().starts_with("invalid request:"));
    let e = decode_frame(r#"{"jsonrpc":"1.0","id":1,"method":"x"}"#).unwrap_err();
    assert!(e.to_string().contains("unsupported jsonrpc version '1.0'"));
}

#[test]
fn event_kind_serializes_to_snake_case() {
    for (kind, expected) in [
        (EventKind::AgentMessageChunk, "agent_message_chunk"),
        (EventKind::AgentThoughtChunk, "agent_thought_chunk"),
        (EventKind::ToolCall, "tool_call"),
        (EventKind::ToolCallUpdate, "tool_call_update"),
        (EventKind::Plan, "plan"),
        (EventKind::PermissionRequest, "permission_request"),
        (EventKind::UserInputRequest, "user_input_request"),
        (EventKind::Checkpoint, "checkpoint"),
        (EventKind::TerminalOutput, "terminal_output"),
        (EventKind::TerminalExit, "terminal_exit"),
        (EventKind::HarEvent, "har_event"),
        (EventKind::SubagentStatus, "subagent_status"),
        (EventKind::FsChange, "fs_change"),
        (EventKind::TurnStatus, "turn_status"),
        (EventKind::SessionStatus, "session_status"),
        (EventKind::TelemetryResources, "telemetry_resources"),
        (EventKind::Error, "error"),
    ] {
        assert_eq!(
            serde_json::to_string(&kind).unwrap(),
            format!("\"{expected}\"")
        );
        assert_eq!(
            serde_json::from_str::<EventKind>(&format!("\"{expected}\"")).unwrap(),
            kind
        );
    }
}
