//! Honest JSON-RPC stubs. Catalog and echo only; no CDP or PTY.

use multiplexer_wire::codec::encode_frame;
use multiplexer_wire::error::{standard, RpcError};
use multiplexer_wire::jsonrpc::{ErrorResponse, Id, Message, Request, Response};
use serde_json::{json, Map, Value};

/// Static catalog matching the desktop seed (`grok`, `grok-4.6`, `fake`).
pub(crate) fn model_list(req: Request) -> Vec<String> {
    vec![ok_frame(
        req.id,
        json!({ "models": ["grok", "grok-4.6", "fake"] }),
    )]
}

/// Ack a nonempty `model` string. Selection is not persisted on the server.
pub(crate) fn model_select(req: Request) -> Vec<String> {
    match parse_model(&req.params) {
        Ok(_) => vec![ok_frame(req.id, json!({ "ok": true }))],
        Err(error) => vec![error_frame(req.id, error)],
    }
}

/// Local snapshot only. No account, no billing, no live token meter.
pub(crate) fn telemetry_usage(req: Request) -> Vec<String> {
    vec![ok_frame(
        req.id,
        json!({
            "turns": 0,
            "tokens": 0,
            "note": "local snapshot only",
        }),
    )]
}

/// This process is the only remote. No Tailscale detect, no Serve, no tickets.
pub(crate) fn remote_list(req: Request) -> Vec<String> {
    vec![ok_frame(
        req.id,
        json!({
            "remotes": [{
                "id": "local",
                "kind": "local",
                "label": "this machine",
            }],
        }),
    )]
}

/// Local threads only. Subagents stay empty. Spawn remains method-not-found.
pub(crate) fn orchestration_list(req: Request, thread_count: usize) -> Vec<String> {
    vec![ok_frame(
        req.id,
        json!({
            "threads": thread_count,
            "subagents": [],
            "note": "Local threads only. Subagent spawn is not wired.",
        }),
    )]
}

/// Honest empty listing. The desktop walks the tree via `multiplexer-client`.
pub(crate) fn fs_list(req: Request) -> Vec<String> {
    vec![ok_frame(
        req.id,
        json!({
            "entries": [],
            "note": "client lists via multiplexer-client",
        }),
    )]
}

fn parse_model(params: &Value) -> Result<String, RpcError> {
    require_nonempty_string(require_object(params)?, "model")
}

fn require_object(params: &Value) -> Result<&Map<String, Value>, RpcError> {
    params
        .as_object()
        .ok_or_else(|| RpcError::new(standard::INVALID_PARAMS, "params must be an object"))
}

fn require_nonempty_string(obj: &Map<String, Value>, field: &str) -> Result<String, RpcError> {
    match obj.get(field) {
        Some(Value::String(s)) if !s.is_empty() => Ok(s.clone()),
        Some(Value::String(_)) => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be non-empty"),
        )),
        Some(_) => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be a string"),
        )),
        None => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("missing {field}"),
        )),
    }
}

fn ok_frame(id: Id, result: Value) -> String {
    encode_frame(&Message::Response(Response::new(id, result))).expect("wire types always encode")
}

fn error_frame(id: Id, error: RpcError) -> String {
    encode_frame(&Message::Error(ErrorResponse::new(id, error))).expect("wire types always encode")
}
