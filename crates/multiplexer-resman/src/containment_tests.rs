use super::windows_job::{
    job_err, pid_is_alive, query_still_active, working_set_limit, JobContainment, PAGE,
};
use super::{ChildId, Containment, ContainmentError, SpawnSpec};
use std::path::PathBuf;

#[test]
fn fake_default_matches_new() {
    let mut c = crate::FakeContainment::default();
    let child = c
        .spawn(SpawnSpec {
            program: PathBuf::from("x"),
            args: vec![],
            memory_cap_bytes: None,
        })
        .unwrap();
    assert_eq!(child.pid, 0);
    assert!(c.child_alive(child.id).unwrap());
    let watch = c.watch();
    let id = child.id;
    c.close();
    assert!(!watch.child_alive(id).unwrap());
}

#[test]
fn sub_page_caps_are_skipped() {
    assert_eq!(working_set_limit(0), None);
    assert_eq!(working_set_limit(1), None);
    assert_eq!(working_set_limit(PAGE - 1), None);
}

#[test]
fn page_aligned_and_unaligned_caps() {
    assert_eq!(working_set_limit(PAGE), Some(PAGE as usize));
    assert_eq!(working_set_limit(PAGE + 1), Some(PAGE as usize));
    assert_eq!(working_set_limit(PAGE * 2 - 1), Some(PAGE as usize));
    assert_eq!(working_set_limit(PAGE * 2), Some((PAGE * 2) as usize));
}

#[test]
fn missing_pid_is_not_alive() {
    assert!(!pid_is_alive(0xFFFF_FFFE));
}

#[test]
fn query_still_active_requires_success_and_259() {
    assert!(!query_still_active(0, 259));
    assert!(!query_still_active(0, 0));
    assert!(query_still_active(1, 259));
    assert!(!query_still_active(1, 0));
    assert!(!query_still_active(1, 1));
}

fn missing_program() -> SpawnSpec {
    SpawnSpec {
        program: PathBuf::from("no-such-multiplexer-child.exe"),
        args: vec![],
        memory_cap_bytes: None,
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
fn job_err_wraps_win32_message() {
    let err = job_err(win32job::JobError::CreateFailed(
        std::io::Error::from_raw_os_error(5),
    ));
    match err {
        ContainmentError::Job(msg) => {
            assert!(msg.contains("Failed to create job"), "{msg}");
        }
        other => panic!("expected Job, got {other:?}"),
    }
}

#[test]
fn spawn_missing_program_is_spawn_error() {
    let mut job = JobContainment::new().expect("create job object");
    match job.spawn(missing_program()) {
        Err(ContainmentError::Spawn { program, message }) => {
            assert!(program.contains("no-such-multiplexer-child"), "{program}");
            assert!(!message.is_empty());
        }
        other => panic!("expected Spawn, got {other:?}"),
    }
}

#[test]
fn child_alive_false_after_quick_exit() {
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut job = JobContainment::new().expect("create job object");
    let child = job
        .spawn(SpawnSpec {
            program: PathBuf::from(system_root).join("System32").join("cmd.exe"),
            args: vec!["/c".into(), "exit".into(), "0".into()],
            memory_cap_bytes: None,
        })
        .expect("spawn cmd /c exit");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut alive = true;
    while std::time::Instant::now() < deadline {
        alive = job.child_alive(child.id).expect("known child");
        if !alive {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!alive, "cmd /c exit must be reported dead");
}

#[test]
fn child_alive_true_after_real_spawn() {
    let mut job = JobContainment::new().expect("create job object");
    let child = job.spawn(long_ping_spec()).expect("spawn ping");
    assert!(job.child_alive(child.id).expect("known child"));
    drop(job);
}

#[test]
fn spawn_after_force_closed_is_closed_and_kills_child() {
    let mut job = JobContainment::new().expect("create job object");
    job.force_closed();
    assert_eq!(
        job.spawn(long_ping_spec()).err(),
        Some(ContainmentError::Closed)
    );
    assert_eq!(
        job.child_alive(ChildId(1)).err(),
        Some(ContainmentError::UnknownChild(1))
    );
}

#[test]
fn memory_cap_after_force_closed_is_closed_before_spawn() {
    let mut job = JobContainment::new().expect("create job object");
    job.force_closed();
    let mut spec = long_ping_spec();
    spec.memory_cap_bytes = Some(8 * 1024 * 1024);
    assert_eq!(job.spawn(spec).err(), Some(ContainmentError::Closed));
    assert_eq!(job.last_working_set_limit(), Some(8 * 1024 * 1024));
}
