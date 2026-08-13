//! posix_openpt backend so macOS and Linux host a shell inside Multiplexer.

use std::collections::VecDeque;
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crate::{TerminalError, TerminalSpec};

/// Interactive child whose stdio is a Unix PTY master.
pub struct EmbeddedSession {
    writer: Option<File>,
    child: Option<libc::pid_t>,
    master_fd: Option<RawFd>,
    pid: u32,
    cols: u16,
    rows: u16,
    output: Arc<Mutex<VecDeque<u8>>>,
    reader: Option<JoinHandle<()>>,
    dead: bool,
}

impl EmbeddedSession {
    /// Open a PTY and exec `program` on the slave.
    pub fn spawn(program: &str, args: &[&str], spec: &TerminalSpec) -> Result<Self, TerminalError> {
        if spec.cols == 0 || spec.rows == 0 {
            return Err(TerminalError::Io(
                "cols and rows must be greater than 0".into(),
            ));
        }
        let (master, slave_name) = open_pty()?;
        set_winsize(master.as_raw(), spec.cols, spec.rows)?;

        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(spawn_err(
                program,
                format!("fork: {}", std::io::Error::last_os_error()),
            ));
        }
        if pid == 0 {
            drop(master);
            child_exec(program, args, spec.cwd.as_path(), &slave_name);
        }

        let output = Arc::new(Mutex::new(VecDeque::new()));
        let queue = Arc::clone(&output);
        let master_fd = master.as_raw();
        let read_file = master
            .try_clone()
            .map_err(|err| TerminalError::Io(format!("clone master: {err}")))?;
        let reader = thread::spawn(move || drain_bytes(read_file, queue));

        Ok(Self {
            writer: Some(master),
            child: Some(pid),
            master_fd: Some(master_fd),
            pid: pid as u32,
            cols: spec.cols,
            rows: spec.rows,
            output,
            reader: Some(reader),
            dead: false,
        })
    }

    pub fn try_read(&mut self) -> Vec<u8> {
        let _ = self.poll_exit();
        let mut queue = self.output.lock().expect("pty output queue");
        queue.drain(..).collect()
    }

    pub fn try_read_str(&mut self) -> String {
        String::from_utf8_lossy(&self.try_read()).into_owned()
    }

    pub fn write(&mut self, data: &[u8]) -> Result<(), TerminalError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| TerminalError::Io("pty input closed".into()))?;
        writer
            .write_all(data)
            .and_then(|_| writer.flush())
            .map_err(|err| TerminalError::Io(format!("write: {err}")))
    }

    pub fn write_str(&mut self, text: &str) -> Result<(), TerminalError> {
        self.write(text.as_bytes())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        let fd = self
            .master_fd
            .ok_or_else(|| TerminalError::Io("pty closed".into()))?;
        set_winsize(fd, cols, rows)?;
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    pub fn kill(&mut self) -> Result<(), TerminalError> {
        if self.dead {
            return Ok(());
        }
        self.dead = true;
        self.writer.take();
        self.master_fd.take();
        if let Some(pid) = self.child.take() {
            unsafe {
                libc::kill(pid, libc::SIGHUP);
                libc::kill(pid, libc::SIGTERM);
                let mut status = 0;
                libc::waitpid(pid, &mut status, 0);
            }
        }
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    pub fn pid(&self) -> Option<u32> {
        if self.pid == 0 {
            None
        } else {
            Some(self.pid)
        }
    }

    pub fn is_alive(&mut self) -> bool {
        !self.poll_exit()
    }

    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    fn poll_exit(&mut self) -> bool {
        let Some(pid) = self.child else {
            return true;
        };
        let mut status = 0;
        let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        r != 0
    }
}

impl Drop for EmbeddedSession {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

fn as_raw(file: &File) -> RawFd {
    file.as_raw()
}

trait AsRaw {
    fn as_raw(&self) -> RawFd;
}

impl AsRaw for File {
    fn as_raw(&self) -> RawFd {
        use std::os::fd::AsRawFd;
        self.as_raw_fd()
    }
}

fn open_pty() -> Result<(File, String), TerminalError> {
    unsafe {
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        if master < 0 {
            return Err(TerminalError::Unsupported(format!(
                "posix_openpt: {}",
                std::io::Error::last_os_error()
            )));
        }
        if libc::grantpt(master) != 0 || libc::unlockpt(master) != 0 {
            let err = std::io::Error::last_os_error();
            libc::close(master);
            return Err(TerminalError::Io(format!("grantpt/unlockpt: {err}")));
        }
        let name = slave_name(master)?;
        Ok((File::from_raw_fd(master), name))
    }
}

#[cfg(target_os = "linux")]
fn slave_name(master: RawFd) -> Result<String, TerminalError> {
    let mut buf = [0u8; 128];
    let rc = unsafe { libc::ptsname_r(master, buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return Err(TerminalError::Io(format!(
            "ptsname_r: {}",
            std::io::Error::last_os_error()
        )));
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8(buf[..end].to_vec())
        .map_err(|err| TerminalError::Io(format!("ptsname: {err}")))
}

#[cfg(not(target_os = "linux"))]
fn slave_name(master: RawFd) -> Result<String, TerminalError> {
    let ptr = unsafe { libc::ptsname(master) };
    if ptr.is_null() {
        return Err(TerminalError::Io(format!(
            "ptsname: {}",
            std::io::Error::last_os_error()
        )));
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
    Ok(cstr.to_string_lossy().into_owned())
}

fn set_winsize(fd: RawFd, cols: u16, rows: u16) -> Result<(), TerminalError> {
    let ws = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let rc = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ, &ws) };
    if rc != 0 {
        return Err(TerminalError::Io(format!(
            "TIOCSWINSZ: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn child_exec(program: &str, args: &[&str], cwd: &Path, slave_name: &str) -> ! {
    unsafe {
        let _ = libc::setsid();
        let slave_c =
            CString::new(slave_name).unwrap_or_else(|_| CString::new("/dev/null").unwrap());
        let slave = libc::open(slave_c.as_ptr(), libc::O_RDWR);
        if slave >= 0 {
            #[cfg(target_os = "linux")]
            {
                let _ = libc::ioctl(slave, libc::TIOCSCTTY, 0);
            }
            let _ = libc::dup2(slave, 0);
            let _ = libc::dup2(slave, 1);
            let _ = libc::dup2(slave, 2);
            if slave > 2 {
                libc::close(slave);
            }
        }
        if let Ok(dir) = CString::new(cwd.to_string_lossy().as_bytes()) {
            let _ = libc::chdir(dir.as_ptr());
        }
        let prog = CString::new(program).unwrap_or_else(|_| CString::new("/bin/sh").unwrap());
        let mut argv = Vec::new();
        argv.push(prog.clone());
        let extras: Vec<CString> = args.iter().filter_map(|a| CString::new(*a).ok()).collect();
        argv.extend(extras);
        let mut ptrs: Vec<*const libc::c_char> = argv.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        libc::execvp(prog.as_ptr(), ptrs.as_ptr());
        libc::_exit(127);
    }
}

fn spawn_err(program: &str, message: String) -> TerminalError {
    TerminalError::Spawn {
        program: program.to_owned(),
        message,
    }
}

fn drain_bytes(mut file: File, queue: Arc<Mutex<VecDeque<u8>>>) {
    let mut buf = [0u8; 4096];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                queue.lock().expect("pty output queue").extend(&buf[..n]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

// Keep IntoRawFd referenced so the child path can own fds if we add more later.
#[allow(dead_code)]
fn forget_fd(file: File) -> RawFd {
    file.into_raw_fd()
}
