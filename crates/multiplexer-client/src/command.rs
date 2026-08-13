//! One `program [args...]` job off the UI thread (cwd set).

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Name of the worker that runs [`spawn_command`].
pub const SHELL_WORKER_THREAD_NAME: &str = "mux-shell-cmd";

/// Inputs for one shell command. `program` is overridable (not hardcoded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
}

/// Captured child stdio plus process success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub ok: bool,
}

impl CommandResult {
    /// Map a finished child to [`CommandResult`]. Lossy UTF-8, no trim.
    pub fn from_output(output: &Output) -> Self {
        Self {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            ok: output.status.success(),
        }
    }
}

impl From<io::Error> for CommandResult {
    fn from(err: io::Error) -> Self {
        Self {
            stdout: String::new(),
            stderr: format!("spawn cmd: {err}"),
            ok: false,
        }
    }
}

/// Windows helper: `cmd.exe /C <line>` in `cwd`.
pub fn windows_cmd(line: &str, cwd: impl Into<PathBuf>) -> CommandRequest {
    CommandRequest {
        program: PathBuf::from("cmd.exe"),
        args: vec![OsString::from("/C"), OsString::from(line)],
        cwd: cwd.into(),
    }
}

/// Start a named thread that runs one command and sends a single [`CommandResult`].
///
/// `program` is overridable so tests never depend on a real shell binary.
pub fn spawn_command(req: CommandRequest) -> Receiver<CommandResult> {
    spawn_command_job(req, run_command)
}

fn spawn_command_job<F>(req: CommandRequest, run: F) -> Receiver<CommandResult>
where
    F: FnOnce(CommandRequest) -> CommandResult + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    thread::Builder::new()
        .name(SHELL_WORKER_THREAD_NAME.to_owned())
        .spawn(move || {
            let _ = tx.send(run(req));
        })
        .expect("start mux-shell-cmd worker");
    rx
}

fn run_command(req: CommandRequest) -> CommandResult {
    result_from_command(
        Command::new(&req.program)
            .args(&req.args)
            .current_dir(&req.cwd)
            .output(),
    )
}

fn result_from_command(output: io::Result<Output>) -> CommandResult {
    match output {
        Ok(output) => CommandResult::from_output(&output),
        Err(err) => CommandResult::from(err),
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

    #[test]
    fn worker_name_constant() {
        assert_eq!(SHELL_WORKER_THREAD_NAME, "mux-shell-cmd");
        assert_ne!(SHELL_WORKER_THREAD_NAME, "mux-grok-turn");
        assert_ne!(SHELL_WORKER_THREAD_NAME, "");
    }

    #[test]
    fn windows_cmd_argv() {
        let req = windows_cmd("git status", PathBuf::from("C:\\repo"));
        assert_eq!(req.program, PathBuf::from("cmd.exe"));
        assert_eq!(
            req.args,
            vec![OsString::from("/C"), OsString::from("git status")]
        );
        assert_eq!(req.cwd, PathBuf::from("C:\\repo"));
        assert_ne!(req.program, PathBuf::from("git"));
        assert_ne!(req.args, vec![OsString::from("git status")]);
    }

    #[test]
    fn from_output_success_and_failure() {
        let ok = CommandResult::from_output(&Output {
            status: exit_status(0),
            stdout: b"  out-text\n".to_vec(),
            stderr: b"  err-text\n".to_vec(),
        });
        assert_eq!(
            ok,
            CommandResult {
                stdout: "  out-text\n".into(),
                stderr: "  err-text\n".into(),
                ok: true,
            }
        );
        assert_ne!(ok.stdout, ok.stderr);
        assert!(ok.ok);

        let fail = CommandResult::from_output(&Output {
            status: failed_status(),
            stdout: Vec::new(),
            stderr: b"boom".to_vec(),
        });
        assert!(!fail.ok);
        assert_eq!(fail.stdout, "");
        assert_eq!(fail.stderr, "boom");

        let lossy = CommandResult::from_output(&Output {
            status: exit_status(0),
            stdout: vec![0xff, b'x'],
            stderr: vec![0xff, b'y'],
        });
        assert!(lossy.stdout.contains('x'));
        assert!(lossy.stderr.contains('y'));
        assert_ne!(lossy.stdout, lossy.stderr);
        assert!(lossy.ok);
    }

    #[test]
    fn spawn_io_error_is_not_ok() {
        let got = CommandResult::from(io::Error::new(io::ErrorKind::NotFound, "missing"));
        assert_eq!(got.stdout, "");
        assert_eq!(got.stderr, "spawn cmd: missing");
        assert!(!got.ok);

        let via_result =
            result_from_command(Err(io::Error::new(io::ErrorKind::NotFound, "missing")));
        assert!(!via_result.ok);
        assert_eq!(via_result.stderr, "spawn cmd: missing");
        assert!(via_result.stdout.is_empty());

        let via_ok = result_from_command(Ok(Output {
            status: exit_status(0),
            stdout: b"yes".to_vec(),
            stderr: Vec::new(),
        }));
        assert_eq!(via_ok.stdout, "yes");
        assert!(via_ok.ok);
    }

    #[test]
    fn fake_runner_sends_one_result_off_caller_thread() {
        let caller = thread::current().id();
        let rx = spawn_command_job(windows_cmd("dir", PathBuf::from(".")), move |req| {
            assert_ne!(thread::current().id(), caller);
            assert_eq!(req.program, PathBuf::from("cmd.exe"));
            CommandResult {
                stdout: "fake-out".into(),
                stderr: String::new(),
                ok: true,
            }
        });
        let got = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("fake worker sends one CommandResult");
        assert_eq!(got.stdout, "fake-out");
        assert!(got.ok);
        assert!(rx.try_recv().ok().is_none());
    }

    #[test]
    fn missing_program_is_not_ok() {
        let rx = spawn_command(CommandRequest {
            program: PathBuf::from("__multiplexer_client_no_such_cmd__"),
            args: Vec::new(),
            cwd: PathBuf::from("."),
        });
        let got = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("worker sends one CommandResult");
        assert!(!got.ok);
        assert!(got.stdout.is_empty());
        assert!(got.stderr.contains("spawn cmd:"));
        assert!(rx.try_recv().ok().is_none());
    }
}
