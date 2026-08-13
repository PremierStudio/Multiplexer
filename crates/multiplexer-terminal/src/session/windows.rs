//! Windows ConPTY backend: `CreatePseudoConsole` + attached `CreateProcessW`.

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read, Write};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};

use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, INVALID_HANDLE_VALUE, S_OK, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
#[cfg(test)]
use windows_sys::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole,
};
use windows_sys::Win32::System::Console::{COORD, HPCON};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess, GetProcessId,
    InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

use crate::cmdline::command_line;
use crate::{TerminalError, TerminalSpec};

const STILL_ACTIVE: u32 = 259;

type CreatePseudoConsoleFn =
    unsafe extern "system" fn(COORD, HANDLE, HANDLE, u32, *mut HPCON) -> windows_sys::core::HRESULT;
type ResizePseudoConsoleFn = unsafe extern "system" fn(HPCON, COORD) -> windows_sys::core::HRESULT;
type ClosePseudoConsoleFn = unsafe extern "system" fn(HPCON);

struct ConptyApi {
    create: CreatePseudoConsoleFn,
    resize: ResizePseudoConsoleFn,
    close: ClosePseudoConsoleFn,
}

/// Interactive child whose stdio is a Windows pseudo console.
pub struct EmbeddedSession {
    con: Option<PseudoConsole>,
    writer: Option<File>,
    process: Option<ProcessHandle>,
    pid: u32,
    cols: u16,
    rows: u16,
    output: Arc<Mutex<VecDeque<u8>>>,
    reader: Option<JoinHandle<()>>,
    dead: bool,
}

struct ProcessHandle(HANDLE);

// HANDLE is a raw pointer. The kernel object is owned only by this session.
unsafe impl Send for ProcessHandle {}

struct PseudoConsole {
    handle: HPCON,
    close: ClosePseudoConsoleFn,
}

unsafe impl Send for PseudoConsole {}

impl Drop for PseudoConsole {
    fn drop(&mut self) {
        // SAFETY: `handle` came from CreatePseudoConsole and is closed once.
        unsafe {
            (self.close)(self.handle);
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        close_handle(self.0);
    }
}

impl EmbeddedSession {
    /// Create a ConPTY of `spec` size and spawn `program` attached to it.
    ///
    /// Host interactive `grok` with empty `args` (no `-p`). Tests use `cmd.exe`.
    pub fn spawn(program: &str, args: &[&str], spec: &TerminalSpec) -> Result<Self, TerminalError> {
        let size = coord(spec.cols, spec.rows)?;
        let api = conpty_api()?;

        let (pty_in_read, host_write) = anonymous_pipe()?;
        let (host_read, pty_out_write) = anonymous_pipe()?;

        let mut hpc: HPCON = 0;
        // SAFETY: pipe handles are open, `hpc` is a valid out-pointer.
        let hr = unsafe {
            (api.create)(
                size,
                pty_in_read.as_raw(),
                pty_out_write.as_raw(),
                0,
                &mut hpc,
            )
        };
        if hr != S_OK {
            return Err(TerminalError::Spawn {
                program: program.to_owned(),
                message: format!("CreatePseudoConsole HRESULT 0x{hr:08X}"),
            });
        }
        // CreatePseudoConsole duplicates the ConPTY-side ends. Drop ours.
        drop(pty_in_read);
        drop(pty_out_write);

        let con = PseudoConsole {
            handle: hpc,
            close: api.close,
        };

        let pi = match spawn_attached(program, args, spec, hpc) {
            Ok(pi) => pi,
            Err(err) => {
                drop(con);
                return Err(err);
            }
        };

        // SAFETY: `pi.hProcess` is a valid process handle from CreateProcessW.
        let pid = unsafe { GetProcessId(pi.hProcess) };
        close_handle(pi.hThread);

        let output = Arc::new(Mutex::new(VecDeque::new()));
        let queue = Arc::clone(&output);
        let read_file = host_read.into_file();
        let reader = thread::spawn(move || drain_bytes(read_file, queue));

        Ok(Self {
            con: Some(con),
            writer: Some(host_write.into_file()),
            process: Some(ProcessHandle(pi.hProcess)),
            pid,
            cols: spec.cols,
            rows: spec.rows,
            output,
            reader: Some(reader),
            dead: false,
        })
    }

    /// Nonblocking drain of bytes read from the ConPTY output pipe so far.
    pub fn try_read(&mut self) -> Vec<u8> {
        let _ = self.poll_exit();
        let mut queue = self.output.lock().expect("conpty output queue");
        queue.drain(..).collect()
    }

    /// Nonblocking drain decoded as UTF-8 (lossy).
    pub fn try_read_str(&mut self) -> String {
        String::from_utf8_lossy(&self.try_read()).into_owned()
    }

    /// Write raw keystroke / paste bytes into the ConPTY input pipe.
    pub fn write(&mut self, data: &[u8]) -> Result<(), TerminalError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| TerminalError::Io("conpty input closed".into()))?;
        writer
            .write_all(data)
            .and_then(|_| writer.flush())
            .map_err(|err| TerminalError::Io(format!("write: {err}")))
    }

    /// Write UTF-8 text. Send `\r` for Enter, not only `\n`.
    pub fn write_str(&mut self, text: &str) -> Result<(), TerminalError> {
        self.write(text.as_bytes())
    }

    /// `ResizePseudoConsole` to `cols` x `rows`.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), TerminalError> {
        let size = coord(cols, rows)?;
        let con = self
            .con
            .as_ref()
            .ok_or_else(|| TerminalError::Io("conpty closed".into()))?;
        let api = conpty_api()?;
        // SAFETY: `con.handle` is a live HPCON from CreatePseudoConsole.
        let hr = unsafe { (api.resize)(con.handle, size) };
        if hr != S_OK {
            return Err(TerminalError::Io(format!(
                "ResizePseudoConsole HRESULT 0x{hr:08X}"
            )));
        }
        self.cols = cols;
        self.rows = rows;
        Ok(())
    }

    /// Close the ConPTY, terminate the child if it is still running. Idempotent.
    pub fn kill(&mut self) -> Result<(), TerminalError> {
        if self.dead {
            return Ok(());
        }
        self.dead = true;
        self.writer.take();
        self.con.take();
        if let Some(proc) = self.process.take() {
            terminate_and_wait(proc.0);
        }
        if let Some(handle) = self.reader.take() {
            let _ = handle.join();
        }
        Ok(())
    }

    /// OS process id of the attached child, if spawn succeeded.
    pub fn pid(&self) -> Option<u32> {
        if self.pid == 0 {
            None
        } else {
            Some(self.pid)
        }
    }

    /// True while the child has not exited.
    pub fn is_alive(&mut self) -> bool {
        !self.poll_exit()
    }

    /// Last successful size (spawn or resize).
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    fn poll_exit(&mut self) -> bool {
        let Some(proc) = self.process.as_ref() else {
            return true;
        };
        // SAFETY: process handle is owned and still open.
        let wait = unsafe { WaitForSingleObject(proc.0, 0) };
        if wait == WAIT_OBJECT_0 {
            return true;
        }
        if wait == WAIT_TIMEOUT {
            return false;
        }
        let mut code = 0u32;
        // SAFETY: same live process handle.
        let ok = unsafe { GetExitCodeProcess(proc.0, &mut code) };
        ok != 0 && code != STILL_ACTIVE
    }
}

impl Drop for EmbeddedSession {
    fn drop(&mut self) {
        let _ = self.kill();
    }
}

fn terminate_and_wait(handle: HANDLE) {
    // SAFETY: `handle` is a process handle we own. Already-exited is ignored.
    unsafe {
        let _ = TerminateProcess(handle, 1);
        let _ = WaitForSingleObject(handle, 5_000);
    }
}

fn close_handle(handle: HANDLE) {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return;
    }
    // SAFETY: handle is a kernel object we own; CloseHandle is idempotent-enough
    // because we only call this after taking ownership and forgetting Drop.
    unsafe {
        let _ = CloseHandle(handle);
    }
}

fn coord(cols: u16, rows: u16) -> Result<COORD, TerminalError> {
    let (cols, rows) = crate::validate_pty_size(cols, rows)?;
    Ok(COORD {
        X: cols as i16,
        Y: rows as i16,
    })
}

fn conpty_api() -> Result<&'static ConptyApi, TerminalError> {
    static API: OnceLock<Result<ConptyApi, String>> = OnceLock::new();
    match API.get_or_init(load_conpty) {
        Ok(api) => Ok(api),
        Err(msg) => Err(TerminalError::Unsupported(msg.clone())),
    }
}

fn load_conpty() -> Result<ConptyApi, String> {
    let kernel = wide("kernel32.dll");
    // SAFETY: `kernel` is a NUL-terminated wide string. kernel32 is always loaded.
    let module = unsafe { GetModuleHandleW(kernel.as_ptr()) };
    if module.is_null() {
        return Err("GetModuleHandleW(kernel32.dll) failed".into());
    }
    let create = proc_addr(module, b"CreatePseudoConsole\0")?;
    let resize = proc_addr(module, b"ResizePseudoConsole\0")?;
    let close = proc_addr(module, b"ClosePseudoConsole\0")?;
    Ok(ConptyApi {
        // SAFETY: GetProcAddress resolved these exports; signatures match Win32.
        create: unsafe { std::mem::transmute::<Farproc, CreatePseudoConsoleFn>(create) },
        resize: unsafe { std::mem::transmute::<Farproc, ResizePseudoConsoleFn>(resize) },
        close: unsafe { std::mem::transmute::<Farproc, ClosePseudoConsoleFn>(close) },
    })
}

type Farproc = unsafe extern "system" fn() -> isize;

fn proc_addr(
    module: windows_sys::Win32::Foundation::HMODULE,
    name: &[u8],
) -> Result<unsafe extern "system" fn() -> isize, String> {
    // SAFETY: `name` is a static NUL-terminated ASCII symbol.
    let addr = unsafe { GetProcAddress(module, name.as_ptr()) };
    addr.ok_or_else(|| {
        let label = String::from_utf8_lossy(&name[..name.len().saturating_sub(1)]);
        format!("{label} not found in kernel32.dll (need Windows 10 1809 or later)")
    })
}

struct PipeEnd(HANDLE);

// HANDLE is a raw pointer. Ownership is exclusive to this wrapper.
unsafe impl Send for PipeEnd {}

impl PipeEnd {
    fn as_raw(&self) -> HANDLE {
        self.0
    }

    fn into_file(self) -> File {
        let handle = self.0;
        std::mem::forget(self);
        // SAFETY: `handle` is an open pipe end; File takes exclusive ownership.
        unsafe { File::from_raw_handle(handle as RawHandle) }
    }
}

impl Drop for PipeEnd {
    fn drop(&mut self) {
        close_handle(self.0);
    }
}

fn anonymous_pipe() -> Result<(PipeEnd, PipeEnd), TerminalError> {
    let mut read = std::ptr::null_mut();
    let mut write = std::ptr::null_mut();
    // SAFETY: out-pointers are valid locals; no security descriptor.
    let ok = unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) };
    if ok == 0 {
        return Err(TerminalError::Io(format!(
            "CreatePipe: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok((PipeEnd(read), PipeEnd(write)))
}

fn spawn_attached(
    program: &str,
    args: &[&str],
    spec: &TerminalSpec,
    hpc: HPCON,
) -> Result<PROCESS_INFORMATION, TerminalError> {
    let mut cmdline = wide(&command_line(program, args));
    let cwd = wide_os(spec.cwd.as_os_str());

    let mut attrs = AttrList::new()?;
    let attr_ptr = attrs.as_ptr();
    // SAFETY: `attr_ptr` is an initialized list; `hpc` is a live HPCON.
    // The documented ConPTY sample passes the HPCON value as lpValue.
    let updated = unsafe {
        UpdateProcThreadAttribute(
            attr_ptr,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            hpc as *const std::ffi::c_void,
            std::mem::size_of::<HPCON>(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if updated == 0 {
        return Err(spawn_err(
            program,
            format!(
                "UpdateProcThreadAttribute: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }

    let mut si: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    si.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
    si.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
    si.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
    si.lpAttributeList = attr_ptr;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: cmdline/cwd are writable NUL-terminated wide buffers; si stays
    // alive for the call; attribute list holds the ConPTY handle.
    let created = unsafe {
        CreateProcessW(
            std::ptr::null(),
            cmdline.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT,
            std::ptr::null(),
            cwd.as_ptr(),
            &si.StartupInfo,
            &mut pi,
        )
    };
    drop(attrs);

    if created == 0 {
        return Err(spawn_err(
            program,
            format!("CreateProcessW: {}", std::io::Error::last_os_error()),
        ));
    }
    Ok(pi)
}

struct AttrList {
    data: Vec<u8>,
    live: bool,
}

impl AttrList {
    fn new() -> Result<Self, TerminalError> {
        let mut bytes_required: usize = 0;
        // SAFETY: size probe; NULL list is required by the API.
        unsafe {
            InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut bytes_required);
        }
        if bytes_required == 0 {
            return Err(TerminalError::Io(
                "InitializeProcThreadAttributeList size was 0".into(),
            ));
        }
        let mut data = vec![0u8; bytes_required];
        let ok = unsafe {
            InitializeProcThreadAttributeList(data.as_mut_ptr().cast(), 1, 0, &mut bytes_required)
        };
        if ok == 0 {
            return Err(TerminalError::Io(format!(
                "InitializeProcThreadAttributeList: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok(Self { data, live: true })
    }

    fn as_ptr(&mut self) -> windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST {
        self.data.as_mut_ptr().cast()
    }
}

impl Drop for AttrList {
    fn drop(&mut self) {
        if self.live {
            // SAFETY: list was initialized and not deleted yet.
            unsafe {
                DeleteProcThreadAttributeList(self.data.as_mut_ptr().cast());
            }
            self.live = false;
        }
    }
}

fn spawn_err(program: &str, message: String) -> TerminalError {
    TerminalError::Spawn {
        program: program.to_owned(),
        message,
    }
}

fn wide(s: &str) -> Vec<u16> {
    wide_os(OsStr::new(s))
}

fn wide_os(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn drain_bytes(mut file: File, queue: Arc<Mutex<VecDeque<u8>>>) {
    let mut buf = [0u8; 4096];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                queue.lock().expect("conpty output queue").extend(&buf[..n]);
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coord_rejects_zero_and_overflow() {
        assert!(coord(0, 24).is_err());
        assert!(coord(80, 0).is_err());
        assert!(coord(80, 24).is_ok());
        assert!(coord(i16::MAX as u16, 1).is_ok());
        assert!(coord(i16::MAX as u16 + 1, 1).is_err());
        assert_ne!(coord(80, 24).unwrap().X, 0);
    }

    #[test]
    fn create_resize_close_are_exported_from_kernel32() {
        let api = load_conpty().expect("CreatePseudoConsole must exist on this Windows");
        // Compare against the statically linked symbols so we did not invent names.
        assert_eq!(api.create as usize, CreatePseudoConsole as usize);
        assert_eq!(api.resize as usize, ResizePseudoConsole as usize);
        assert_eq!(api.close as usize, ClosePseudoConsole as usize);
    }
}
