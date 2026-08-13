//! Parse and reply helpers for `git.worktree.create`.

use std::sync::Mutex;

use multiplexer_wire::codec::encode_frame;
use multiplexer_wire::error::{standard, AppErrorKind, RpcError};
use multiplexer_wire::jsonrpc::{ErrorResponse, Id, Message, Request, Response};
use serde_json::{json, Map, Value};

use crate::backend::BackendError;
use crate::git::GitCatalog;

/// `git.worktree.create` against an optional catalog (parent dispatch target).
pub(crate) fn create(slot: &Mutex<Option<Box<dyn GitCatalog>>>, req: Request) -> Vec<String> {
    let result = parse_create(&req.params).and_then(|(cwd, path, branch, create_branch)| {
        let git = slot.lock().unwrap_or_else(|p| p.into_inner());
        match git.as_ref() {
            Some(catalog) => catalog
                .create_worktree(&cwd, &path, &branch, create_branch)
                .map(|info| json!({ "worktree": info }))
                .map_err(backend_rpc),
            None => Err(RpcError::app(
                AppErrorKind::Unsupported,
                "git catalog not configured",
            )),
        }
    });
    reply(req.id, result)
}

pub(crate) fn parse_create(params: &Value) -> Result<(String, String, String, bool), RpcError> {
    let obj = require_object(params)?;
    let cwd = require_nonempty_string(obj, "cwd")?;
    let path = require_nonempty_string(obj, "path")?;
    let branch = require_nonempty_string(obj, "branch")?;
    let create_branch = optional_bool(obj, "create_branch")?;
    Ok((cwd, path, branch, create_branch))
}

#[allow(dead_code)]
pub(crate) fn reply_ok(id: Id, result: Value) -> Vec<String> {
    reply(id, Ok(result))
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

fn optional_bool(obj: &Map<String, Value>, field: &str) -> Result<bool, RpcError> {
    match obj.get(field) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(v)) => Ok(*v),
        Some(_) => Err(RpcError::new(
            standard::INVALID_PARAMS,
            format!("{field} must be a boolean"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use multiplexer_wire::codec::decode_frame;
    use multiplexer_worktree::{FakeGit, WorktreeError, WorktreeService};
    use serde_json::json;

    #[test]
    fn parse_missing_field() {
        let err = parse_create(&json!({ "path": "/wt", "branch": "feat" })).unwrap_err();
        assert_eq!(err.code, standard::INVALID_PARAMS);
        assert_eq!(err.message, "missing cwd");

        let err = parse_create(&json!({ "cwd": "/repo", "branch": "feat" })).unwrap_err();
        assert_eq!(err.code, standard::INVALID_PARAMS);
        assert_eq!(err.message, "missing path");

        let err = parse_create(&json!({ "cwd": "/repo", "path": "/wt" })).unwrap_err();
        assert_eq!(err.code, standard::INVALID_PARAMS);
        assert_eq!(err.message, "missing branch");
    }

    #[test]
    fn parse_create_branch_default_false() {
        let parsed = parse_create(&json!({
            "cwd": "/repo",
            "path": "/wt",
            "branch": "feat"
        }))
        .expect("parse");
        assert_eq!(parsed, ("/repo".into(), "/wt".into(), "feat".into(), false));
    }

    #[test]
    fn parse_true() {
        let parsed = parse_create(&json!({
            "cwd": "/repo",
            "path": "/wt",
            "branch": "feat",
            "create_branch": true
        }))
        .expect("parse");
        assert_eq!(parsed, ("/repo".into(), "/wt".into(), "feat".into(), true));
    }

    #[test]
    fn reply_ok_encodes_response() {
        let frames = reply_ok(Id::String("1".into()), json!({ "ok": true }));
        assert_eq!(frames.len(), 1);
        match decode_frame(&frames[0]).expect("frame") {
            Message::Response(resp) => {
                assert_eq!(resp.id, Id::String("1".into()));
                assert_eq!(resp.result, json!({ "ok": true }));
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    fn catalog_slot(git: FakeGit) -> Mutex<Option<Box<dyn GitCatalog>>> {
        Mutex::new(Some(Box::new(WorktreeService::new(git))))
    }

    fn create_req(params: Value) -> Request {
        Request::new(Id::String("c1".into()), "git.worktree.create", params)
    }

    #[test]
    fn create_replies_with_listed_worktree() {
        let git = FakeGit::new();
        git.push(Ok(String::new()));
        git.push(Ok(
            "worktree /wt\nHEAD def456\nbranch refs/heads/feat\n".into()
        ));
        let frames = create(
            &catalog_slot(git),
            create_req(json!({
                "cwd": "/repo",
                "path": "/wt",
                "branch": "feat"
            })),
        );
        match decode_frame(&frames[0]).expect("frame") {
            Message::Response(resp) => {
                assert_eq!(resp.id, Id::String("c1".into()));
                assert_eq!(resp.result["worktree"]["path"], "/wt");
                assert_eq!(resp.result["worktree"]["head"], "def456");
                assert_eq!(resp.result["worktree"]["branch"], "refs/heads/feat");
            }
            other => panic!("expected response, got {other:?}"),
        }
    }

    #[test]
    fn create_maps_runner_error() {
        let git = FakeGit::new();
        git.push(Err(WorktreeError::Git("add failed".into())));
        let frames = create(
            &catalog_slot(git),
            create_req(json!({
                "cwd": "/repo",
                "path": "/wt",
                "branch": "feat"
            })),
        );
        match decode_frame(&frames[0]).expect("frame") {
            Message::Error(resp) => {
                assert_eq!(resp.error.code, AppErrorKind::ProviderError.code());
                assert!(resp.error.message.contains("add failed"));
            }
            other => panic!("expected error, got {other:?}"),
        }
    }
}
