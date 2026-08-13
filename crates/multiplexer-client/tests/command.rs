//! Construction and field tests. These do not invoke grok or an OS shell.
//!
//! `CommandRequest.program` is the fake hook. Live `cmd.exe` spawn is
//! OS-specific; skip it here. Parent should run this crate (see TEST PLAN).

use std::ffi::OsString;
use std::path::PathBuf;

use multiplexer_client::{windows_cmd, CommandRequest, CommandResult, SHELL_WORKER_THREAD_NAME};

#[test]
fn windows_cmd_argv() {
    let req = windows_cmd("dir", "C:\\work\\app");
    assert_eq!(req.program, PathBuf::from("cmd.exe"));
    assert_eq!(req.args, vec![OsString::from("/C"), OsString::from("dir")]);
    assert_eq!(req.cwd, PathBuf::from("C:\\work\\app"));
    assert_ne!(req.program, PathBuf::from("cmd"));
}

#[test]
fn command_request_stores_overridable_program() {
    let req = CommandRequest {
        program: PathBuf::from("__multiplexer_client_no_such_cmd__"),
        args: vec![OsString::from("status")],
        cwd: PathBuf::from("."),
    };
    assert_eq!(
        req.program,
        PathBuf::from("__multiplexer_client_no_such_cmd__")
    );
    assert_ne!(req.program, PathBuf::from("cmd.exe"));
    assert_eq!(req.args, vec![OsString::from("status")]);
}

#[test]
fn command_result_success_and_failure_fields() {
    let ok = CommandResult {
        stdout: "On branch main\n".into(),
        stderr: String::new(),
        ok: true,
    };
    assert!(ok.ok);
    assert_eq!(ok.stdout, "On branch main\n");
    assert!(ok.stderr.is_empty());

    let fail = CommandResult {
        stdout: String::new(),
        stderr: "spawn cmd: not found".into(),
        ok: false,
    };
    assert!(!fail.ok);
    assert!(fail.stdout.is_empty());
    assert_eq!(fail.stderr, "spawn cmd: not found");
    assert_ne!(ok, fail);
}

#[test]
fn io_error_maps_to_spawn_cmd_prefix() {
    let result = CommandResult::from(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "the system cannot find the file specified",
    ));
    assert!(!result.ok);
    assert!(result.stdout.is_empty());
    assert!(result.stderr.starts_with("spawn cmd:"));
    assert_ne!(result.stderr, "the system cannot find the file specified");
}

#[test]
fn worker_name_constant() {
    assert_eq!(SHELL_WORKER_THREAD_NAME, "mux-shell-cmd");
}
