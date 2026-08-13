//! `terminal.*` JSON-RPC handlers backed by [`TerminalHub`].

use std::path::PathBuf;
use std::sync::Mutex;

use multiplexer_terminal::{TerminalError, TerminalHub, TerminalId, TerminalSpec};
use multiplexer_wire::codec::encode_frame;
use multiplexer_wire::error::{standard, AppErrorKind, RpcError};
use multiplexer_wire::jsonrpc::{ErrorResponse, Id, Message, Request, Response};
use serde_json::{json, Map, Value};

pub(crate) fn create(slot: &Mutex<Option<TerminalHub>>, req: Request) -> Vec<String> {
    let result = parse_create(&req.params).and_then(|spec| {
        with_hub(slot, |hub| {
            let id = hub.create(spec);
            Ok(json!({ "id": id.as_str() }))
        })
    });
    reply(req.id, result)
}

pub(crate) fn list(slot: &Mutex<Option<TerminalHub>>, req: Request) -> Vec<String> {
    let result = with_hub(slot, |hub| {
        let terminals: Vec<Value> = hub
            .list()
            .into_iter()
            .map(|id| {
                json!({
                    "id": id.as_str(),
                    "alive": hub.is_alive(&id),
                })
            })
            .collect();
        Ok(json!({ "terminals": terminals }))
    });
    reply(req.id, result)
}

pub(crate) fn input(slot: &Mutex<Option<TerminalHub>>, req: Request) -> Vec<String> {
    let result = parse_input(&req.params).and_then(|(id, data)| {
        with_hub(slot, |hub| {
            hub.input(&id, data.as_bytes()).map_err(term_rpc)?;
            Ok(json!({}))
        })
    });
    reply(req.id, result)
}

pub(crate) fn kill(slot: &Mutex<Option<TerminalHub>>, req: Request) -> Vec<String> {
    let result = parse_id(&req.params).and_then(|id| {
        with_hub(slot, |hub| {
            hub.kill(&id).map_err(term_rpc)?;
            Ok(json!({}))
        })
    });
    reply(req.id, result)
}

fn with_hub<R>(
    slot: &Mutex<Option<TerminalHub>>,
    f: impl FnOnce(&mut TerminalHub) -> Result<R, RpcError>,
) -> Result<R, RpcError> {
    let mut guard = slot.lock().unwrap_or_else(|p| p.into_inner());
    let hub = guard
        .as_mut()
        .ok_or_else(|| RpcError::app(AppErrorKind::Unsupported, "terminal hub not configured"))?;
    f(hub)
}

fn parse_create(params: &Value) -> Result<TerminalSpec, RpcError> {
    let obj = require_object(params)?;
    let cols = require_u16(obj, "cols")?;
    let rows = require_u16(obj, "rows")?;
    let cwd = match optional_string(obj, "cwd")? {
        Some(cwd) => PathBuf::from(cwd),
        None => PathBuf::from("."),
    };
    Ok(TerminalSpec::new(cols, rows, cwd))
}

fn parse_input(params: &Value) -> Result<(TerminalId, String), RpcError> {
    let obj = require_object(params)?;
    let id = TerminalId::from(require_nonempty_string(obj, "id")?);
    let data = match obj.get("data") {
        Some(Value::String(s)) => s.clone(),
        Some(_) => {
            return Err(RpcError::new(
                standard::INVALID_PARAMS,
                "data must be a string",
            ));
        }
        None => {
            return Err(RpcError::new(standard::INVALID_PARAMS, "missing data"));
        }
    };
    Ok((id, data))
}

fn parse_id(params: &Value) -> Result<TerminalId, RpcError> {
    Ok(TerminalId::from(require_nonempty_string(
        require_object(params)?,
        "id",
    )?))
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

fn optional_string(obj: &Map<String, Value>, field: &str) -> Result<Option<String>, RpcError> {
    match obj.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be non-empty"),
        )),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be a string"),
        )),
    }
}

fn require_u16(obj: &Map<String, Value>, field: &str) -> Result<u16, RpcError> {
    match obj.get(field) {
        Some(Value::Number(n)) => match n.as_u64().and_then(|v| u16::try_from(v).ok()) {
            Some(v) => Ok(v),
            None => Err(RpcError::new(
                standard::INVALID_PARAMS,
                format!("{field} must be a u16"),
            )),
        },
        Some(_) => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be a number"),
        )),
        None => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("missing {field}"),
        )),
    }
}

fn term_rpc(err: TerminalError) -> RpcError {
    match err {
        TerminalError::NotFound(id) => {
            RpcError::app(AppErrorKind::NotFound, format!("terminal not found: {id}"))
        }
        TerminalError::Spawn { program, message } => RpcError::app(
            AppErrorKind::ProviderError,
            format!("spawn `{program}`: {message}"),
        ),
        TerminalError::Io(message) => RpcError::app(AppErrorKind::ProviderError, message),
    }
}

fn reply(id: Id, result: Result<Value, RpcError>) -> Vec<String> {
    match result {
        Ok(value) => vec![ok_frame(id, value)],
        Err(error) => vec![error_frame(id, error)],
    }
}

fn ok_frame(id: Id, result: Value) -> String {
    encode_frame(&Message::Response(Response::new(id, result))).expect("wire types always encode")
}

fn error_frame(id: Id, error: RpcError) -> String {
    encode_frame(&Message::Error(ErrorResponse::new(id, error))).expect("wire types always encode")
}
