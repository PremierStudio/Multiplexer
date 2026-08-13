//! Construction and field tests. These do not invoke grok or an OS shell.
//!
//! `TurnRequest.program` is the fake hook. Live spawn with `cmd` / `echo` is
//! OS-specific; skip it here. Parent should run this crate (see TEST PLAN).

use std::path::PathBuf;
use std::sync::mpsc;

use multiplexer_client::{try_recv, TurnError, TurnRequest, TurnResult, WORKER_THREAD_NAME};

#[test]
fn turn_request_stores_overridable_program_cwd_and_prompt() {
    let req = TurnRequest {
        cwd: PathBuf::from("C:\\work\\app"),
        prompt: "summarize this repo".into(),
        program: PathBuf::from("C:\\tools\\fake-grok.exe"),
    };
    assert_eq!(req.cwd, PathBuf::from("C:\\work\\app"));
    assert_eq!(req.prompt, "summarize this repo");
    assert_eq!(req.program, PathBuf::from("C:\\tools\\fake-grok.exe"));
    assert_ne!(req.program, PathBuf::from("grok"));
    assert_ne!(req.prompt, "");
    assert_eq!(
        req.program_args(),
        vec![
            std::ffi::OsString::from("--always-approve"),
            std::ffi::OsString::from("--cwd"),
            std::ffi::OsString::from("C:\\work\\app"),
            std::ffi::OsString::from("-p"),
            std::ffi::OsString::from("summarize this repo"),
        ]
    );
}

#[test]
fn turn_result_success_fields() {
    let result = TurnResult {
        stdout: "assistant text".into(),
        stderr: String::new(),
        ok: true,
    };
    assert_eq!(result.stdout, "assistant text");
    assert_eq!(result.stderr, "");
    assert!(result.ok);
    assert_ne!(
        result,
        TurnResult {
            stdout: "assistant text".into(),
            stderr: String::new(),
            ok: false,
        }
    );
}

#[test]
fn turn_result_failure_fields() {
    let result = TurnResult {
        stdout: String::new(),
        stderr: "spawn grok: not found".into(),
        ok: false,
    };
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, "spawn grok: not found");
    assert!(!result.ok);
    assert_ne!(result.stdout, result.stderr);
}

#[test]
fn try_recv_none_when_empty() {
    let (_tx, rx) = mpsc::channel::<TurnResult>();
    assert!(try_recv(&rx).is_none());
}

#[test]
fn try_recv_some_when_ready_then_none() {
    let (tx, rx) = mpsc::channel();
    tx.send(TurnResult {
        stdout: "out".into(),
        stderr: "err".into(),
        ok: true,
    })
    .unwrap();
    let got = try_recv(&rx).expect("one queued TurnResult");
    assert_eq!(got.stdout, "out");
    assert_eq!(got.stderr, "err");
    assert!(got.ok);
    assert!(try_recv(&rx).is_none());
}

#[test]
fn try_recv_none_when_disconnected() {
    let (tx, rx) = mpsc::channel::<TurnResult>();
    drop(tx);
    assert!(try_recv(&rx).is_none());
}

#[test]
fn turn_error_spawn_display_is_prefixed() {
    let err = TurnError::Spawn(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "the system cannot find the file specified",
    ));
    let text = err.to_string();
    assert!(text.starts_with("spawn grok:"));
    assert_ne!(text, "the system cannot find the file specified");
    let result = TurnResult::from(err);
    assert!(!result.ok);
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, text);
}

#[test]
fn worker_thread_name_literal() {
    assert_eq!(WORKER_THREAD_NAME, "mux-grok-turn");
}
