//! One `program --always-approve --cwd <cwd> -p <prompt>` job off the UI thread.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Name of the worker that runs [`spawn_grok_turn`].
pub const WORKER_THREAD_NAME: &str = "mux-grok-turn";

const FLAG_ALWAYS_APPROVE: &str = "--always-approve";
const FLAG_CWD: &str = "--cwd";
const FLAG_PROMPT: &str = "-p";

/// Failure starting the child process.
#[derive(Debug, thiserror::Error)]
pub enum TurnError {
    /// `CreateProcess` / exec of [`TurnRequest::program`] failed.
    #[error("spawn grok: {0}")]
    Spawn(#[source] io::Error),
}

/// Inputs for one headless grok turn. `program` is overridable (not hardcoded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnRequest {
    pub cwd: PathBuf,
    pub prompt: String,
    pub program: PathBuf,
}

impl TurnRequest {
    /// argv after `program`: `--always-approve --cwd <cwd> -p <prompt>`.
    pub fn program_args(&self) -> Vec<OsString> {
        vec![
            OsString::from(FLAG_ALWAYS_APPROVE),
            OsString::from(FLAG_CWD),
            self.cwd.as_os_str().to_os_string(),
            OsString::from(FLAG_PROMPT),
            OsString::from(&self.prompt),
        ]
    }
}

/// Captured child stdio plus process success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnResult {
    pub stdout: String,
    pub stderr: String,
    pub ok: bool,
}

impl TurnResult {
    /// Map a finished child to [`TurnResult`]. Lossy UTF-8, no trim.
    pub fn from_output(output: &Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            ok: output.status.success(),
        }
    }
}

impl From<TurnError> for TurnResult {
    fn from(err: TurnError) -> Self {
        Self {
            stdout: String::new(),
            stderr: err.to_string(),
            ok: false,
        }
    }
}

/// Start a named thread that runs one turn and sends a single [`TurnResult`].
pub fn spawn_grok_turn(req: TurnRequest) -> Receiver<TurnResult> {
    spawn_turn_job(req, run_grok_turn)
}

/// Non-blocking poll for the UI frame loop. Empty and disconnected are `None`.
pub fn try_recv(rx: &Receiver<TurnResult>) -> Option<TurnResult> {
    rx.try_recv().ok()
}

fn spawn_turn_job<F>(req: TurnRequest, run: F) -> Receiver<TurnResult>
where
    F: FnOnce(TurnRequest) -> TurnResult + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name(WORKER_THREAD_NAME.to_owned())
        .spawn(move || {
            let _ = tx.send(run(req));
        })
        .expect("start mux-grok-turn worker");
    rx
}

fn run_grok_turn(req: TurnRequest) -> TurnResult {
    let mut cmd = Command::new(&req.program);
    for arg in req.program_args() {
        cmd.arg(arg);
    }
    result_from_command(cmd.output())
}

fn result_from_command(output: io::Result<Output>) -> TurnResult {
    match output {
        Ok(output) => TurnResult::from_output(&output),
        Err(err) => TurnResult::from(TurnError::Spawn(err)),
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use std::process::ExitStatus;
    use std::time::Duration;

    #[cfg(windows)]
    fn exit_status(code: u32) -> ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(code)
    }

    #[cfg(unix)]
    fn exit_status(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code)
    }

    #[cfg(windows)]
    fn failed_status() -> ExitStatus {
        exit_status(1)
    }

    #[cfg(unix)]
    fn failed_status() -> ExitStatus {
        exit_status(1 << 8)
    }

    fn sample_req() -> TurnRequest {
        TurnRequest {
            cwd: PathBuf::from("C:\\repo"),
            prompt: "hello world".into(),
            program: PathBuf::from("C:\\tools\\fake-grok.exe"),
        }
    }

    #[test]
    fn worker_thread_name_is_mux_grok_turn() {
        assert_eq!(WORKER_THREAD_NAME, "mux-grok-turn");
        assert_ne!(WORKER_THREAD_NAME, "grok");
        assert_ne!(WORKER_THREAD_NAME, "");
    }

    #[test]
    fn program_args_are_always_approve_cwd_and_prompt() {
        let req = sample_req();
        assert_eq!(
            req.program_args(),
            vec![
                OsString::from("--always-approve"),
                OsString::from("--cwd"),
                OsString::from("C:\\repo"),
                OsString::from("-p"),
                OsString::from("hello world"),
            ]
        );
        assert_ne!(req.program, PathBuf::from("grok"));
    }

    #[test]
    fn from_output_success_keeps_stdio_and_ok() {
        let output = Output {
            status: exit_status(0),
            stdout: b"out-text".to_vec(),
            stderr: b"err-text".to_vec(),
        };
        let got = TurnResult::from_output(&output);
        assert_eq!(
            got,
            TurnResult {
                stdout: "out-text".into(),
                stderr: "err-text".into(),
                ok: true,
            }
        );
        assert_ne!(got.stdout, got.stderr);
        assert!(got.ok);
    }

    #[test]
    fn from_output_failure_sets_ok_false() {
        let output = Output {
            status: failed_status(),
            stdout: Vec::new(),
            stderr: b"boom".to_vec(),
        };
        let got = TurnResult::from_output(&output);
        assert!(!got.ok);
        assert_eq!(got.stdout, "");
        assert_eq!(got.stderr, "boom");
    }

    #[test]
    fn from_output_is_lossy_utf8() {
        let output = Output {
            status: exit_status(0),
            stdout: vec![0xff, b'x'],
            stderr: vec![0xff, b'y'],
        };
        let got = TurnResult::from_output(&output);
        assert!(got.stdout.contains('x'));
        assert!(got.stderr.contains('y'));
        assert_ne!(got.stdout, got.stderr);
        assert!(got.ok);
    }

    #[test]
    fn turn_error_spawn_maps_to_failed_result() {
        let err = TurnError::Spawn(io::Error::new(io::ErrorKind::NotFound, "missing"));
        assert_eq!(err.to_string(), "spawn grok: missing");
        let got = TurnResult::from(err);
        assert_eq!(got.stdout, "");
        assert_eq!(got.stderr, "spawn grok: missing");
        assert!(!got.ok);
    }

    #[test]
    fn result_from_command_ok_and_err() {
        let ok = result_from_command(Ok(Output {
            status: exit_status(0),
            stdout: b"yes".to_vec(),
            stderr: Vec::new(),
        }));
        assert_eq!(ok.stdout, "yes");
        assert!(ok.ok);
        let err = result_from_command(Err(io::Error::new(io::ErrorKind::NotFound, "missing")));
        assert!(!err.ok);
        assert_eq!(err.stderr, "spawn grok: missing");
        assert!(err.stdout.is_empty());
    }

    #[test]
    fn fake_runner_sends_one_result_off_caller_thread() {
        let caller = thread::current().id();
        let rx = spawn_turn_job(sample_req(), move |req| {
            assert_ne!(thread::current().id(), caller);
            assert_eq!(req.prompt, "hello world");
            TurnResult {
                stdout: "fake-out".into(),
                stderr: String::new(),
                ok: true,
            }
        });
        let got = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("fake worker sends one TurnResult");
        assert_eq!(got.stdout, "fake-out");
        assert!(got.ok);
        assert!(try_recv(&rx).is_none());
    }

    #[test]
    fn missing_program_is_failed_spawn_not_ok() {
        let rx = spawn_grok_turn(TurnRequest {
            cwd: PathBuf::from("."),
            prompt: "hi".into(),
            program: PathBuf::from("__multiplexer_client_no_such_program__"),
        });
        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("worker sends one TurnResult");
        assert!(!got.ok);
        assert!(got.stdout.is_empty());
        assert!(got.stderr.contains("spawn grok:"));
        assert!(try_recv(&rx).is_none());
    }
}
