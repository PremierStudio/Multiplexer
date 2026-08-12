//! Request router: decode a JSON-RPC frame, dispatch, encode a response.

use std::sync::{Mutex, MutexGuard};

use multiplexer_wire::codec::{decode_frame, encode_frame, CodecError};
use multiplexer_wire::error::{standard, AppErrorKind, RpcError};
use multiplexer_wire::jsonrpc::{ErrorResponse, Id, Message, Request, Response};
use multiplexer_wire::methods;
use multiplexer_wire::protocol::PROTOCOL_VERSION;
use serde_json::{json, Map, Value};

use crate::backend::{BackendError, FakeBackend, SessionBackend, SessionStartParams};

/// In-process JSON-RPC router. Holds the session backend behind a mutex.
pub struct Server<B = FakeBackend> {
    backend: Mutex<B>,
}

impl Server<FakeBackend> {
    pub fn new() -> Self {
        Self::with_backend(FakeBackend::new())
    }
}

impl Server<crate::ProviderBridge<multiplexer_provider::FakeProvider>> {
    /// Router backed by the shared [`FakeProvider`].
    pub fn with_fake_provider() -> Self {
        Self::with_backend(crate::ProviderBridge::fake())
    }
}

impl Default for Server<FakeBackend> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: SessionBackend> Server<B> {
    pub fn with_backend(backend: B) -> Self {
        Self {
            backend: Mutex::new(backend),
        }
    }

    /// Decode `text`, dispatch, and return the response frame plus any
    /// `event` notification frames.
    pub fn handle_frame(&self, text: &str) -> Vec<String> {
        match decode_frame(text) {
            Ok(Message::Request(req)) => self.dispatch(req),
            Ok(Message::Notification(_)) | Ok(Message::Response(_)) | Ok(Message::Error(_)) => {
                Vec::new()
            }
            Err(CodecError::Parse(msg)) => {
                vec![error_frame(
                    correlation_id(text),
                    RpcError::new(standard::PARSE_ERROR, msg),
                )]
            }
            Err(CodecError::InvalidRequest(msg)) => {
                vec![error_frame(
                    correlation_id(text),
                    RpcError::new(standard::INVALID_REQUEST, msg),
                )]
            }
        }
    }

    fn dispatch(&self, req: Request) -> Vec<String> {
        match req.method.as_str() {
            methods::SESSION_START => self.session_start(req),
            methods::SESSION_LIST => self.session_list(req),
            methods::SESSION_GET => self.session_get(req),
            methods::SESSION_STOP => self.session_stop(req),
            methods::TURN_SEND => self.turn_send(req),
            methods::SYSTEM_PING => vec![ok_frame(req.id, json!({ "pong": true }))],
            methods::SYSTEM_HELLO => self.system_hello(req),
            _ => vec![error_frame(
                req.id,
                RpcError::new(
                    standard::METHOD_NOT_FOUND,
                    format!("method not found: {}", req.method),
                ),
            )],
        }
    }

    fn session_start(&self, req: Request) -> Vec<String> {
        let params = match parse_start(&req.params) {
            Ok(p) => p,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let mut backend = self.lock();
        match backend.start(params) {
            Ok(started) => respond_and_drain(
                &mut *backend,
                req.id,
                json!({ "session_id": started.session_id }),
            ),
            Err(e) => vec![error_frame(req.id, backend_rpc(e))],
        }
    }

    fn session_list(&self, req: Request) -> Vec<String> {
        let sessions = self.lock().list();
        vec![ok_frame(req.id, json!({ "sessions": sessions }))]
    }

    fn session_get(&self, req: Request) -> Vec<String> {
        let session_id = match parse_session_id(&req.params) {
            Ok(id) => id,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        match self.lock().get(&session_id) {
            Ok(snap) => vec![ok_frame(
                req.id,
                serde_json::to_value(snap).expect("snapshot encodes"),
            )],
            Err(e) => vec![error_frame(req.id, backend_rpc(e))],
        }
    }

    fn session_stop(&self, req: Request) -> Vec<String> {
        let session_id = match parse_session_id(&req.params) {
            Ok(id) => id,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        match self.lock().stop(&session_id) {
            Ok(()) => vec![ok_frame(req.id, json!({}))],
            Err(e) => vec![error_frame(req.id, backend_rpc(e))],
        }
    }

    fn turn_send(&self, req: Request) -> Vec<String> {
        let (session_id, text) = match parse_turn(&req.params) {
            Ok(pair) => pair,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let mut backend = self.lock();
        match backend.send_turn(&session_id, &text) {
            Ok(()) => respond_and_drain(&mut *backend, req.id, json!({ "accepted": true })),
            Err(e) => vec![error_frame(req.id, backend_rpc(e))],
        }
    }

    fn system_hello(&self, req: Request) -> Vec<String> {
        if let Some(obj) = req.params.as_object() {
            match obj.get("protocol_version") {
                Some(Value::String(v)) if v.as_str() != PROTOCOL_VERSION => {
                    return vec![error_frame(
                        req.id,
                        RpcError::app(
                            AppErrorKind::ProtocolVersionMismatch,
                            format!("server protocol_version is {PROTOCOL_VERSION}"),
                        ),
                    )];
                }
                Some(Value::String(_)) | None => {}
                Some(_) => {
                    return vec![error_frame(
                        req.id,
                        RpcError::new(
                            standard::INVALID_PARAMS,
                            "protocol_version must be a string",
                        ),
                    )];
                }
            }
        }
        vec![ok_frame(
            req.id,
            json!({
                "server_info": {
                    "name": "multiplexer-server",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "protocol_version": PROTOCOL_VERSION,
            }),
        )]
    }

    fn lock(&self) -> MutexGuard<'_, B> {
        self.backend.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn respond_and_drain<B: SessionBackend>(backend: &mut B, id: Id, result: Value) -> Vec<String> {
    let mut frames = vec![ok_frame(id.clone(), result)];
    frames.extend(drain_notifications(backend, &id));
    frames
}

fn drain_notifications<B: SessionBackend>(backend: &mut B, request_id: &Id) -> Vec<String> {
    backend
        .drain_events()
        .into_iter()
        .map(|mut ev| {
            ev.in_response_to = Some(request_id.clone());
            encode_ok(Message::Notification(ev.to_notification()))
        })
        .collect()
}

fn parse_start(params: &Value) -> Result<SessionStartParams, RpcError> {
    let obj = require_object(params)?;
    Ok(SessionStartParams {
        provider: require_nonempty_string(obj, "provider")?,
        model: require_nonempty_string(obj, "model")?,
        workspace: require_nonempty_string(obj, "workspace")?,
        initial_prompt: optional_string(obj, "initial_prompt")?,
    })
}

fn parse_session_id(params: &Value) -> Result<String, RpcError> {
    require_nonempty_string(require_object(params)?, "session_id")
}

fn parse_turn(params: &Value) -> Result<(String, String), RpcError> {
    let obj = require_object(params)?;
    let session_id = require_nonempty_string(obj, "session_id")?;
    let text = match obj.get("text") {
        Some(Value::String(s)) => s.clone(),
        Some(_) => {
            return Err(RpcError::new(
                standard::INVALID_PARAMS,
                "text must be a string",
            ));
        }
        None => {
            return Err(RpcError::new(standard::INVALID_PARAMS, "missing text"));
        }
    };
    Ok((session_id, text))
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
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be a string"),
        )),
    }
}

fn backend_rpc(err: BackendError) -> RpcError {
    match err {
        BackendError::NotFound { session_id } => RpcError::app(
            AppErrorKind::NotFound,
            format!("session not found: {session_id}"),
        ),
        BackendError::Provider { kind, message } => RpcError::app(kind, message),
    }
}

fn correlation_id(text: &str) -> Id {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Id::Number(0);
    };
    match value.get("id") {
        Some(Value::String(s)) => Id::String(s.clone()),
        Some(Value::Number(n)) => n.as_i64().map(Id::Number).unwrap_or(Id::Number(0)),
        _ => Id::Number(0),
    }
}

fn ok_frame(id: Id, result: Value) -> String {
    encode_ok(Message::Response(Response::new(id, result)))
}

fn error_frame(id: Id, error: RpcError) -> String {
    encode_ok(Message::Error(ErrorResponse::new(id, error)))
}

fn encode_ok(msg: Message) -> String {
    encode_frame(&msg).expect("wire types always encode")
}
