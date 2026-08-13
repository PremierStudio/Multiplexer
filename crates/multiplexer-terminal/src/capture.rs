//! Piped child process with a background line reader.
//!
//! Job assignment is skipped. multiplexer-resman `JobContainment` is not used:
//! its `spawn` forces `Stdio::null` and it has no API to assign an already-piped
//! `std::process::Child`. There is no circular crate edge; the Job API simply
//! cannot keep capture pipes.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::TerminalError;

#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Child process whose stdout and stderr are drained into a line queue.
pub struct ProcessCapture {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Arc<Mutex<VecDeque<String>>>,
    readers: Vec<JoinHandle<()>>,
}

impl ProcessCapture {
    /// Spawn `program` with piped stdin/stdout/stderr in `cwd`.
    pub fn spawn(program: &str, args: &[&str], cwd: &Path) -> Result<Self, TerminalError> {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
        }

        let mut child = command.spawn().map_err(|err| TerminalError::Spawn {
            program: program.to_owned(),
            message: err.to_string(),
        })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdin = child.stdin.take();
        let lines = Arc::new(Mutex::new(VecDeque::new()));
        let mut readers = Vec::new();

        if let Some(out) = stdout {
            let queue = Arc::clone(&lines);
            readers.push(thread::spawn(move || drain_lines(out, queue)));
        }
        if let Some(err) = stderr {
            let queue = Arc::clone(&lines);
            readers.push(thread::spawn(move || drain_lines(err, queue)));
        }

        Ok(Self {
            child,
            stdin,
            lines,
            readers,
        })
    }

    /// Nonblocking drain of lines captured so far (stdout and stderr).
    pub fn try_read(&mut self) -> Vec<String> {
        let _ = self.child.try_wait();
        let mut queue = self.lines.lock().expect("process capture queue");
        queue.drain(..).collect()
    }

    /// Write raw bytes to the child stdin (no extra newline).
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), TerminalError> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| TerminalError::Io("stdin closed".into()))?;
        stdin
            .write_all(bytes)
            .and_then(|_| stdin.flush())
            .map_err(|err| TerminalError::Io(format!("write: {err}")))
    }

    /// Write `line` plus a trailing newline to the child stdin.
    pub fn write_line(&mut self, line: &str) -> Result<(), TerminalError> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| TerminalError::Io("stdin closed".into()))?;
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|err| TerminalError::Io(format!("write: {err}")))
    }

    /// Terminate the child and close stdin. Already-exited is success.
    pub fn kill(&mut self) -> Result<(), TerminalError> {
        self.stdin.take();
        match self.child.kill() {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => {}
            Err(err) => return Err(TerminalError::Io(format!("kill: {err}"))),
        }
        let _ = self.child.wait();
        for handle in self.readers.drain(..) {
            let _ = handle.join();
        }
        Ok(())
    }
}

impl Drop for ProcessCapture {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

fn drain_lines(reader: impl Read, queue: Arc<Mutex<VecDeque<String>>>) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                if buf.last() == Some(&b'\n') {
                    buf.pop();
                }
                if buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                let line = String::from_utf8_lossy(&buf).into_owned();
                queue.lock().expect("process capture queue").push_back(line);
            }
            Err(_) => break,
        }
    }
}
