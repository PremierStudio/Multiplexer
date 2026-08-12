//! Frame codec for JSON-RPC over WebSocket (plan/04 §2, §3).
//!
//! One JSON-RPC message per WebSocket text frame; no length-prefixing needed
//! (WebSocket already frames). This module encodes a `Message` to a UTF-8
//! text frame and decodes a text frame back into a `Message`, validating the
//! version field and the request/response/notification shape rules.

use serde_json::Value;
use thiserror::Error;

use crate::jsonrpc::{
    ErrorResponse, Id, Message, Notification, Request, Response, JSONRPC_VERSION,
};

/// Errors produced while decoding a frame.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CodecError {
    /// The frame is not valid JSON.
    #[error("parse error: {0}")]
    Parse(String),
    /// The frame is valid JSON but not a well-formed JSON-RPC message.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

/// Encode a message into a single UTF-8 text frame.
pub fn encode_frame(msg: &Message) -> Result<String, CodecError> {
    let value = match msg {
        Message::Request(r) => serde_json::to_value(r).map_err(serde_err)?,
        Message::Response(r) => serde_json::to_value(r).map_err(serde_err)?,
        Message::Error(r) => serde_json::to_value(r).map_err(serde_err)?,
        Message::Notification(n) => serde_json::to_value(n).map_err(serde_err)?,
    };
    serde_json::to_string(&value).map_err(serde_err)
}

/// Decode a single UTF-8 text frame into a message.
pub fn decode_frame(text: &str) -> Result<Message, CodecError> {
    let value: Value = serde_json::from_str(text).map_err(|e| CodecError::Parse(e.to_string()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| CodecError::InvalidRequest("frame must be a JSON object".to_owned()))?;

    let version = obj
        .get("jsonrpc")
        .and_then(Value::as_str)
        .ok_or_else(|| CodecError::InvalidRequest("missing jsonrpc version".to_owned()))?;
    if version != JSONRPC_VERSION {
        return Err(CodecError::InvalidRequest(format!(
            "unsupported jsonrpc version '{version}'"
        )));
    }

    let has_method = obj.contains_key("method");
    let has_result = obj.contains_key("result");
    let has_error = obj.contains_key("error");
    let has_id = obj.contains_key("id");

    if has_method {
        // A request or a notification; never both result and error.
        if has_result || has_error {
            return Err(CodecError::InvalidRequest(
                "message with method cannot carry result or error".to_owned(),
            ));
        }
        let method = obj
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| CodecError::InvalidRequest("method must be a string".to_owned()))?;
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        if has_id {
            let id = parse_id(obj)?;
            Ok(Message::Request(Request {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id,
                method: method.to_owned(),
                params,
            }))
        } else {
            Ok(Message::Notification(Notification {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                method: method.to_owned(),
                params,
            }))
        }
    } else {
        // A response or error response; must carry an id.
        if !has_id {
            return Err(CodecError::InvalidRequest(
                "response must carry an id".to_owned(),
            ));
        }
        if has_result && has_error {
            return Err(CodecError::InvalidRequest(
                "response cannot carry both result and error".to_owned(),
            ));
        }
        let id = parse_id(obj)?;
        if has_error {
            let error = serde_json::from_value(obj["error"].clone())
                .map_err(|e| CodecError::InvalidRequest(format!("invalid error object: {e}")))?;
            Ok(Message::Error(ErrorResponse {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id,
                error,
            }))
        } else if has_result {
            Ok(Message::Response(Response {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id,
                result: obj["result"].clone(),
            }))
        } else {
            Err(CodecError::InvalidRequest(
                "message must carry method, result, or error".to_owned(),
            ))
        }
    }
}

fn parse_id(obj: &serde_json::Map<String, Value>) -> Result<Id, CodecError> {
    match &obj["id"] {
        Value::String(s) => Ok(Id::String(s.clone())),
        Value::Number(n) => n
            .as_i64()
            .map(Id::Number)
            .ok_or_else(|| CodecError::InvalidRequest("id must be a string or integer".to_owned())),
        _ => Err(CodecError::InvalidRequest(
            "id must be a string or integer".to_owned(),
        )),
    }
}

fn serde_err(e: serde_json::Error) -> CodecError {
    CodecError::Parse(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_err_wraps_serde_message_as_parse() {
        let src = serde_json::from_str::<u8>("[]").expect_err("type mismatch");
        let err = serde_err(src);
        assert!(matches!(&err, CodecError::Parse(s) if !s.is_empty()));
        assert!(err.to_string().starts_with("parse error:"));
    }
}
