//! Real Windows Job Object kill-on-close (plan/24 D58).

#![cfg(windows)]

use multiplexer_resman::{ChildId, Containment, ContainmentError, JobContainment, SpawnSpec};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// Kill `pid` if the test unwinds before kill-on-close is proven.
struct OrphanGuard {
    pid: u32,
    armed: bool,
}

impl OrphanGuard {
    fn new(pid: u32) -> Self {
        Self { pid, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for OrphanGuard {
    fn drop(&mut self) {
        if self.armed {
            terminate_pid(self.pid);
        }
    }
}

fn long_ping_spec() -> SpawnSpec {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    SpawnSpec {
        program: PathBuf::from(system_root).join("System32").join("ping.exe"),
        args: vec!["-n".into(), "60".into(), "127.0.0.1".into()],
        memory_cap_bytes: None,
    }
}

#[test]
fn job_kill_on_close_reaps_ping() {
    let mut job = JobContainment::new().expect("create job object");
    let child = job.spawn(long_ping_spec()).expect("spawn ping in job");
    let mut guard = OrphanGuard::new(child.pid);

    assert!(child.pid != 0, "JobContainment must return a real OS pid");
    assert!(
        job.child_alive(child.id).expect("known child"),
        "ping must be alive after spawn"
    );
    assert!(
        pid_is_alive(child.pid),
        "OS must see ping pid {} before job drop",
        child.pid
    );

    drop(job);

    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && pid_is_alive(child.pid) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !pid_is_alive(child.pid),
        "kill-on-close must reap ping pid {}",
        child.pid
    );
    guard.disarm();
}

#[test]
fn job_sequential_child_ids() {
    let mut job = JobContainment::new().expect("create job object");
    let first = job.spawn(long_ping_spec()).expect("first ping");
    let mut first_guard = OrphanGuard::new(first.pid);
    let second = job.spawn(long_ping_spec()).expect("second ping");
    let mut second_guard = OrphanGuard::new(second.pid);
    assert_eq!(first.id, ChildId(1));
    assert_eq!(second.id, ChildId(2));
    drop(job);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && (pid_is_alive(first.pid) || pid_is_alive(second.pid)) {
        thread::sleep(Duration::from_millis(50));
    }
    first_guard.disarm();
    second_guard.disarm();
}

#[test]
fn job_applies_page_aligned_memory_cap() {
    let mut job = JobContainment::new().expect("create job object");
    assert_eq!(job.last_working_set_limit(), None);

    let mut tiny = long_ping_spec();
    tiny.memory_cap_bytes = Some(100);
    let child = job.spawn(tiny).expect("spawn with sub-page cap");
    let mut guard = OrphanGuard::new(child.pid);
    assert_eq!(job.last_working_set_limit(), None);
    drop(job);
    guard.disarm();

    let mut job = JobContainment::new().expect("create job object");
    let mut capped = long_ping_spec();
    capped.memory_cap_bytes = Some(8 * 1024 * 1024);
    let child = job.spawn(capped).expect("spawn with 8 MiB cap");
    let mut guard = OrphanGuard::new(child.pid);
    assert_eq!(job.last_working_set_limit(), Some(8 * 1024 * 1024));
    drop(job);
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && pid_is_alive(child.pid) {
        thread::sleep(Duration::from_millis(50));
    }
    guard.disarm();
}

#[test]
fn job_child_alive_false_after_terminate() {
    let mut job = JobContainment::new().expect("create job object");
    let child = job.spawn(long_ping_spec()).expect("spawn ping in job");
    let mut guard = OrphanGuard::new(child.pid);
    assert!(
        job.child_alive(child.id).expect("known child"),
        "ping must be alive after spawn"
    );
    terminate_pid(child.pid);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut alive = true;
    while Instant::now() < deadline {
        alive = job.child_alive(child.id).expect("known child");
        if !alive {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !alive,
        "pid_is_alive must report terminated pid {} as dead",
        child.pid
    );
    guard.disarm();
}

#[test]
fn job_child_alive_false_after_quick_exit() {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut job = JobContainment::new().expect("create job object");
    let child = job
        .spawn(SpawnSpec {
            program: PathBuf::from(system_root).join("System32").join("cmd.exe"),
            args: vec!["/c".into(), "exit".into(), "0".into()],
            memory_cap_bytes: None,
        })
        .expect("spawn cmd /c exit");
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut alive = true;
    while Instant::now() < deadline {
        alive = job.child_alive(child.id).expect("known child");
        if !alive {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive, "cmd /c exit must be reported dead");
}

#[test]
fn job_child_alive_unknown_id_errors() {
    let job = JobContainment::new().expect("create job object");
    assert_eq!(
        job.child_alive(ChildId(99)).err(),
        Some(ContainmentError::UnknownChild(99))
    );
}

#[test]
fn missing_pid_is_not_alive() {
    assert!(!multiplexer_resman::pid_is_alive(0xFFFF_FFFE));
}

#[test]
fn job_err_and_query_helpers() {
    let err = multiplexer_resman::job_err(win32job::JobError::CreateFailed(
        std::io::Error::from_raw_os_error(5),
    ));
    match err {
        ContainmentError::Job(msg) => assert!(msg.contains("Failed to create job"), "{msg}"),
        other => panic!("expected Job, got {other:?}"),
    }
    assert!(!multiplexer_resman::query_still_active(0, 259));
    assert!(multiplexer_resman::query_still_active(1, 259));
    assert!(!multiplexer_resman::query_still_active(1, 0));
    assert_eq!(multiplexer_resman::working_set_limit(0), None);
    assert_eq!(
        multiplexer_resman::working_set_limit(multiplexer_resman::PAGE),
        Some(multiplexer_resman::PAGE as usize)
    );
}

#[test]
fn spawn_missing_program_is_spawn_error() {
    let mut job = JobContainment::new().expect("create job object");
    match job.spawn(SpawnSpec {
        program: PathBuf::from("no-such-multiplexer-child.exe"),
        args: vec![],
        memory_cap_bytes: None,
    }) {
        Err(ContainmentError::Spawn { program, message }) => {
            assert!(program.contains("no-such-multiplexer-child"), "{program}");
            assert!(!message.is_empty());
        }
        other => panic!("expected Spawn, got {other:?}"),
    }
}

#[test]
fn child_alive_true_after_spawn_for_coverage() {
    let mut job = JobContainment::new().expect("create job object");
    let child = job.spawn(long_ping_spec()).expect("spawn ping");
    let mut guard = OrphanGuard::new(child.pid);
    assert!(job.child_alive(child.id).expect("known child"));
    drop(job);
    guard.disarm();
}

#[test]
fn spawn_after_force_closed_is_closed() {
    let mut job = JobContainment::new().expect("create job object");
    job.force_closed();
    assert_eq!(
        job.spawn(long_ping_spec()).err(),
        Some(ContainmentError::Closed)
    );
}

#[test]
fn memory_cap_after_force_closed_is_closed() {
    let mut job = JobContainment::new().expect("create job object");
    job.force_closed();
    let mut spec = long_ping_spec();
    spec.memory_cap_bytes = Some(8 * 1024 * 1024);
    assert_eq!(job.spawn(spec).err(), Some(ContainmentError::Closed));
    assert_eq!(job.last_working_set_limit(), Some(8 * 1024 * 1024));
}

#[test]
fn job_close_reaps_ping() {
    let mut job = JobContainment::new().expect("create job object");
    let child = job.spawn(long_ping_spec()).expect("spawn ping in job");
    let mut guard = OrphanGuard::new(child.pid);
    assert!(pid_is_alive(child.pid));

    job.close();

    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && pid_is_alive(child.pid) {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !pid_is_alive(child.pid),
        "close() must reap ping pid {}",
        child.pid
    );
    guard.disarm();
}

const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_TERMINATE: u32 = 0x0001;
const STILL_ACTIVE: u32 = 259;

fn pid_is_alive(pid: u32) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(handle, &mut code);
        CloseHandle(handle);
        ok != 0 && code == STILL_ACTIVE
    }
}

fn terminate_pid(pid: u32) {
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return;
        }
        let _ = TerminateProcess(handle, 1);
        CloseHandle(handle);
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    fn GetExitCodeProcess(handle: *mut std::ffi::c_void, code: *mut u32) -> i32;
    fn TerminateProcess(handle: *mut std::ffi::c_void, code: u32) -> i32;
}
