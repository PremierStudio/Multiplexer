//! Request router: decode a JSON-RPC frame, dispatch, encode a response.

use std::sync::{Mutex, MutexGuard};

use multiplexer_terminal::TerminalHub;
use multiplexer_wire::approval::ApprovalDecision;
use multiplexer_wire::codec::{decode_frame, encode_frame, CodecError};
use multiplexer_wire::error::{standard, AppErrorKind, RpcError};
use multiplexer_wire::jsonrpc::{ErrorResponse, Id, Message, Request, Response};
use multiplexer_wire::methods;
use multiplexer_wire::protocol::PROTOCOL_VERSION;
use serde_json::{json, Map, Value};

use crate::backend::{BackendError, FakeBackend, SessionBackend, SessionStartParams};
use crate::checkpoints::CheckpointCatalog;
use crate::git::GitCatalog;

/// In-process JSON-RPC router. Holds the session backend behind a mutex.
pub struct Server<B = FakeBackend> {
    backend: Mutex<B>,
    git: Mutex<Option<Box<dyn GitCatalog>>>,
    checkpoints: Mutex<Option<Box<dyn CheckpointCatalog>>>,
    terminals: Mutex<Option<TerminalHub>>,
}

impl Server<FakeBackend> {
    pub fn new() -> Self {
        Self::with_backend(FakeBackend::new())
    }

    /// Router with a scripted (or real) git catalog for `git.worktrees`.
    pub fn with_git<G: GitCatalog + 'static>(git: G) -> Self {
        let server = Self::new();
        server.install_git(git);
        server
    }
}

impl Server<crate::ProviderBridge<multiplexer_provider::FakeProvider>> {
    /// Router backed by the shared [`FakeProvider`].
    pub fn with_fake_provider() -> Self {
        Self::with_backend(crate::ProviderBridge::fake())
    }
}

impl
    Server<
        crate::ProviderBridge<
            multiplexer_provider::GrokAdapter<multiplexer_provider::CliGrokFactory>,
        >,
    >
{
    /// Local machine defaults: real `grok -p` plus real `git worktree`.
    pub fn with_local() -> Self {
        let adapter =
            multiplexer_provider::GrokAdapter::new(multiplexer_provider::CliGrokFactory::new());
        let server = Self::with_backend(crate::ProviderBridge::new(adapter));
        server.install_git(multiplexer_worktree::WorktreeService::new(
            multiplexer_worktree::ProcessGit::new(),
        ));
        server.install_checkpoints(multiplexer_checkpoint::CheckpointStore::new());
        server.install_terminals(multiplexer_terminal::TerminalHub::new());
        server
    }
}

impl Server<crate::RuntimeBackend> {
    /// Router that composes provider, resource manager, and checkpoints.
    pub fn with_runtime() -> Self {
        Self::with_backend(crate::RuntimeBackend::new())
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
            git: Mutex::new(None),
            checkpoints: Mutex::new(None),
            terminals: Mutex::new(None),
        }
    }

    /// Install or replace the git catalog used by `git.worktrees`.
    pub fn install_git<G: GitCatalog + 'static>(&self, git: G) {
        *self.git.lock().unwrap_or_else(|p| p.into_inner()) = Some(Box::new(git));
    }

    /// Install or replace the checkpoint catalog used by `checkpoint.list` / `checkpoint.revert`.
    pub fn install_checkpoints<C: CheckpointCatalog + 'static>(&self, store: C) {
        *self.checkpoints.lock().unwrap_or_else(|p| p.into_inner()) = Some(Box::new(store));
    }

    /// Install or replace the hub used by `terminal.*`.
    pub fn install_terminals(&self, hub: TerminalHub) {
        *self.terminals.lock().unwrap_or_else(|p| p.into_inner()) = Some(hub);
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
            methods::SESSION_INTERRUPT => self.session_interrupt(req),
            methods::TURN_SEND => self.turn_send(req),
            methods::APPROVAL_RESPOND => self.approval_respond(req),
            methods::GIT_WORKTREES => self.git_worktrees(req),
            methods::GIT_WORKTREE_CREATE => crate::worktree_create::create(&self.git, req),
            methods::CHECKPOINT_LIST => self.checkpoint_list(req),
            methods::CHECKPOINT_CREATE => self.checkpoint_create(req),
            methods::CHECKPOINT_REVERT => self.checkpoint_revert(req),
            methods::TERMINAL_CREATE => crate::terms::create(&self.terminals, req),
            methods::TERMINAL_LIST => crate::terms::list(&self.terminals, req),
            methods::TERMINAL_INPUT => crate::terms::input(&self.terminals, req),
            methods::TERMINAL_KILL => crate::terms::kill(&self.terminals, req),
            methods::SYSTEM_PING => vec![ok_frame(req.id, json!({ "pong": true }))],
            methods::SYSTEM_HELLO => self.system_hello(req),
            methods::MODEL_LIST => crate::stubs::model_list(req),
            methods::MODEL_SELECT => crate::stubs::model_select(req),
            methods::TELEMETRY_USAGE => crate::stubs::telemetry_usage(req),
            methods::REMOTE_LIST => crate::stubs::remote_list(req),
            methods::FS_LIST => crate::stubs::fs_list(req),
            methods::ORCHESTRATION_LIST => {
                let n = self.lock().list().len();
                crate::stubs::orchestration_list(req, n)
            }
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

    fn session_interrupt(&self, req: Request) -> Vec<String> {
        let session_id = match parse_session_id(&req.params) {
            Ok(id) => id,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let mut backend = self.lock();
        match backend.interrupt(&session_id) {
            Ok(()) => respond_and_drain(&mut *backend, req.id, json!({})),
            Err(e) => vec![error_frame(req.id, backend_rpc(e))],
        }
    }

    fn approval_respond(&self, req: Request) -> Vec<String> {
        let (session_id, request_id, decision) = match parse_approval(&req.params) {
            Ok(parsed) => parsed,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let mut backend = self.lock();
        match backend.approval_respond(&session_id, &request_id, decision) {
            Ok(()) => respond_and_drain(&mut *backend, req.id, json!({})),
            Err(e) => vec![error_frame(req.id, backend_rpc(e))],
        }
    }

    fn checkpoint_create(&self, req: Request) -> Vec<String> {
        let (session_id, label) = match parse_checkpoint_create(&req.params) {
            Ok(pair) => pair,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let mut store = self.checkpoints.lock().unwrap_or_else(|p| p.into_inner());
        match store.as_mut() {
            Some(catalog) => match catalog.create(&session_id, &label) {
                Ok(checkpoint) => vec![ok_frame(
                    req.id,
                    serde_json::to_value(checkpoint).expect("checkpoint encodes"),
                )],
                Err(e) => vec![error_frame(req.id, backend_rpc(e))],
            },
            None => vec![error_frame(
                req.id,
                RpcError::app(
                    AppErrorKind::Unsupported,
                    "checkpoint catalog not configured",
                ),
            )],
        }
    }

    fn checkpoint_list(&self, req: Request) -> Vec<String> {
        let session_id = match parse_session_id(&req.params) {
            Ok(id) => id,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let store = self.checkpoints.lock().unwrap_or_else(|p| p.into_inner());
        match store.as_ref() {
            Some(catalog) => {
                let checkpoints = catalog.list(&session_id);
                vec![ok_frame(req.id, json!({ "checkpoints": checkpoints }))]
            }
            None => vec![error_frame(
                req.id,
                RpcError::app(
                    AppErrorKind::Unsupported,
                    "checkpoint catalog not configured",
                ),
            )],
        }
    }

    fn checkpoint_revert(&self, req: Request) -> Vec<String> {
        let checkpoint_id = match parse_checkpoint_id(&req.params) {
            Ok(id) => id,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let mut store = self.checkpoints.lock().unwrap_or_else(|p| p.into_inner());
        match store.as_mut() {
            Some(catalog) => match catalog.revert(&checkpoint_id) {
                Ok(checkpoint) => vec![ok_frame(
                    req.id,
                    serde_json::to_value(checkpoint).expect("checkpoint encodes"),
                )],
                Err(e) => vec![error_frame(req.id, backend_rpc(e))],
            },
            None => vec![error_frame(
                req.id,
                RpcError::app(
                    AppErrorKind::Unsupported,
                    "checkpoint catalog not configured",
                ),
            )],
        }
    }

    fn git_worktrees(&self, req: Request) -> Vec<String> {
        let cwd = match parse_cwd(&req.params) {
            Ok(cwd) => cwd,
            Err(e) => return vec![error_frame(req.id, e)],
        };
        let git = self.git.lock().unwrap_or_else(|p| p.into_inner());
        match git.as_ref() {
            Some(catalog) => match catalog.list_worktrees(&cwd) {
                Ok(worktrees) => vec![ok_frame(req.id, json!({ "worktrees": worktrees }))],
                Err(e) => vec![error_frame(req.id, backend_rpc(e))],
            },
            None => vec![error_frame(
                req.id,
                RpcError::app(AppErrorKind::Unsupported, "git catalog not configured"),
            )],
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

fn parse_approval(params: &Value) -> Result<(String, String, ApprovalDecision), RpcError> {
    let obj = require_object(params)?;
    let session_id = require_nonempty_string(obj, "session_id")?;
    let request_id = require_nonempty_string(obj, "request_id")?;
    let raw = require_nonempty_string(obj, "decision")?;
    let decision = ApprovalDecision::parse(&raw)
        .map_err(|err| RpcError::new(standard::INVALID_PARAMS, err.to_string()))?;
    Ok((session_id, request_id, decision))
}

fn parse_cwd(params: &Value) -> Result<String, RpcError> {
    require_nonempty_string(require_object(params)?, "cwd")
}

fn parse_checkpoint_id(params: &Value) -> Result<String, RpcError> {
    require_nonempty_string(require_object(params)?, "checkpoint_id")
}

fn parse_checkpoint_create(params: &Value) -> Result<(String, String), RpcError> {
    let obj = require_object(params)?;
    let session_id = require_nonempty_string(obj, "session_id")?;
    let label = require_nonempty_string(obj, "label")?;
    Ok((session_id, label))
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
