use std::path::PathBuf;

use super::*;
use crate::containment::FakeContainment;

fn dummy_spec() -> SpawnSpec {
    SpawnSpec {
        program: PathBuf::from("dummy"),
        args: vec![],
        memory_cap_bytes: None,
    }
}

#[test]
fn fake_zero_cores_errors() {
    assert!(matches!(
        ResourceManager::fake(0),
        Err(ManagerError::Bitmap(ResmanError::InvalidCoreCount))
    ));
}

#[test]
fn new_zero_cores_errors() {
    assert!(matches!(
        ResourceManager::new(0, FakeContainment::new),
        Err(ManagerError::Bitmap(ResmanError::InvalidCoreCount))
    ));
}

#[test]
fn new_with_fn_factory_succeeds() {
    let mut mgr = ResourceManager::new(8, FakeContainment::new).unwrap();
    mgr.start_session(SessionId(1), 1, dummy_spec()).unwrap();
    assert_eq!(mgr.session_count(), 1);
}

#[test]
fn start_stop_and_queries() {
    let mut mgr = ResourceManager::fake(8).unwrap();
    let alloc = mgr.start_session(SessionId(1), 2, dummy_spec()).unwrap();
    assert_eq!(alloc.cores, vec![2, 3]);
    assert!(mgr.session_alive(SessionId(1)).unwrap());
    assert!(mgr.child_id(SessionId(1)).is_ok());
    assert!(mgr.alloc_of(SessionId(1)).is_some());
    assert_eq!(mgr.session_count(), 1);
    assert_eq!(mgr.free_enabled_count(), 4);
    mgr.stop_session(SessionId(1)).unwrap();
    assert_eq!(mgr.session_count(), 0);
    assert!(matches!(
        mgr.session_alive(SessionId(1)),
        Err(ManagerError::UnknownSession(1))
    ));
    assert!(matches!(
        mgr.child_id(SessionId(9)),
        Err(ManagerError::UnknownSession(9))
    ));
}

struct FailOnce;

impl Containment for FailOnce {
    fn spawn(&mut self, _spec: SpawnSpec) -> Result<ContainedChild, ContainmentError> {
        Err(ContainmentError::Spawn {
            program: "x".into(),
            message: "boom".into(),
        })
    }

    fn child_alive(&self, _id: ChildId) -> Result<bool, ContainmentError> {
        Err(ContainmentError::UnknownChild(0))
    }
}

#[test]
fn fake_empty_program_rolls_back() {
    let mut mgr = ResourceManager::fake(8).unwrap();
    let before = mgr.free_enabled_count();
    let err = mgr
        .start_session(
            SessionId(1),
            1,
            SpawnSpec {
                program: PathBuf::new(),
                args: vec![],
                memory_cap_bytes: None,
            },
        )
        .unwrap_err();
    assert!(matches!(err, ManagerError::Containment(_)));
    assert_eq!(mgr.free_enabled_count(), before);
    assert_eq!(mgr.session_count(), 0);
}

#[test]
fn failed_spawn_rolls_back() {
    let mut mgr = ResourceManager::new(8, || FailOnce).unwrap();
    let before = mgr.free_enabled_count();
    assert!(mgr.start_session(SessionId(1), 2, dummy_spec()).is_err());
    assert_eq!(mgr.free_enabled_count(), before);
}

struct AliveErr(FakeContainment);

impl Containment for AliveErr {
    fn spawn(&mut self, spec: SpawnSpec) -> Result<ContainedChild, ContainmentError> {
        self.0.spawn(spec)
    }

    fn child_alive(&self, _id: ChildId) -> Result<bool, ContainmentError> {
        Err(ContainmentError::UnknownChild(0))
    }
}

#[test]
fn session_alive_propagates_error() {
    let mut mgr = ResourceManager::new(8, || AliveErr(FakeContainment::new())).unwrap();
    mgr.start_session(SessionId(1), 1, dummy_spec()).unwrap();
    assert!(matches!(
        mgr.session_alive(SessionId(1)),
        Err(ManagerError::Containment(ContainmentError::UnknownChild(0)))
    ));
}

#[test]
fn manager_error_display() {
    assert!(ManagerError::UnknownSession(1)
        .to_string()
        .contains("unknown session 1"));
}
