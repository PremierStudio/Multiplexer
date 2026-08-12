use multiplexer_server::Server;
use multiplexer_wire::codec::{decode_frame, encode_frame};
use multiplexer_wire::event::EventKind;
use multiplexer_wire::jsonrpc::{Id, Message, Request};
use multiplexer_wire::methods;
use serde_json::{json, Value};

fn rpc(id: &str, method: &str, params: Value) -> String {
    encode_frame(&Message::Request(Request::new(
        Id::String(id.to_owned()),
        method,
        params,
    )))
    .unwrap()
}

fn result_of(frames: &[String]) -> Value {
    for f in frames {
        if let Message::Response(r) = decode_frame(f).unwrap() {
            return r.result;
        }
    }
    panic!("no success response in {frames:?}");
}

fn events(frames: &[String]) -> Vec<EventKind> {
    frames
        .iter()
        .filter_map(|f| match decode_frame(f).unwrap() {
            Message::Notification(n) if n.method == methods::EVENT => {
                let ev: multiplexer_wire::event::StreamEvent =
                    serde_json::from_value(n.params).unwrap();
                Some(ev.event)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn fake_provider_start_uses_sess_one() {
    let server = Server::with_fake_provider();
    let frames = server.handle_frame(&rpc(
        "1",
        methods::SESSION_START,
        json!({
            "provider": "fake",
            "model": "grok",
            "workspace": "C:/tmp/ws",
            "initial_prompt": "hi",
        }),
    ));
    let result = result_of(&frames);
    assert_eq!(result["session_id"], "sess-1");
    let kinds = events(&frames);
    assert!(kinds.contains(&EventKind::SessionStatus));
    assert!(kinds.contains(&EventKind::AgentMessageChunk));
    assert!(kinds.contains(&EventKind::TurnStatus));
}

#[test]
fn fake_provider_list_and_stop() {
    let server = Server::with_fake_provider();
    server.handle_frame(&rpc(
        "1",
        methods::SESSION_START,
        json!({
            "provider": "fake",
            "model": "grok",
            "workspace": "/ws",
        }),
    ));
    let listed = result_of(&server.handle_frame(&rpc("2", methods::SESSION_LIST, json!({}))));
    assert_eq!(listed["sessions"][0]["id"], "sess-1");
    server.handle_frame(&rpc(
        "3",
        methods::SESSION_STOP,
        json!({ "session_id": "sess-1" }),
    ));
    let listed = result_of(&server.handle_frame(&rpc("4", methods::SESSION_LIST, json!({}))));
    assert_eq!(listed["sessions"].as_array().unwrap().len(), 0);
}

#[test]
fn turn_send_echoes_chunk() {
    let server = Server::with_fake_provider();
    server.handle_frame(&rpc(
        "1",
        methods::SESSION_START,
        json!({
            "provider": "fake",
            "model": "grok",
            "workspace": "/ws",
        }),
    ));
    let frames = server.handle_frame(&rpc(
        "2",
        methods::TURN_SEND,
        json!({ "session_id": "sess-1", "text": "hello" }),
    ));
    assert!(events(&frames).contains(&EventKind::AgentMessageChunk));
}

#[test]
fn unknown_session_is_not_found() {
    let server = Server::with_fake_provider();
    let frames = server.handle_frame(&rpc(
        "1",
        methods::SESSION_GET,
        json!({ "session_id": "missing" }),
    ));
    let err = frames.iter().find_map(|f| match decode_frame(f).unwrap() {
        Message::Error(e) => Some(e.error),
        _ => None,
    });
    let err = err.expect("error response");
    assert_eq!(
        err.data.as_ref().map(|d| d.kind.as_str()),
        Some("not_found")
    );
}
