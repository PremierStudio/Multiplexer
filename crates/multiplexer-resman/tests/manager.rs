use std::path::PathBuf;

use multiplexer_resman::{
    ContainmentError, FakeContainment, ResourceManager, SessionId, SpawnSpec,
};

fn dummy_spec() -> SpawnSpec {
    SpawnSpec {
        program: PathBuf::from("dummy"),
        args: vec![],
        memory_cap_bytes: None,
    }
}

#[test]
fn start_allocates_cores_and_child() {
    let mut mgr = ResourceManager::fake(8).unwrap();
    let alloc = mgr.start_session(SessionId(1), 2, dummy_spec()).unwrap();
    assert_eq!(alloc.cores, vec![2, 3]);
    assert_eq!(mgr.alloc_of(SessionId(1)), Some(&alloc));
    assert_eq!(mgr.alloc_of(SessionId(99)), None);
    assert_eq!(mgr.session_count(), 1);
    assert!(mgr.session_alive(SessionId(1)).unwrap());
    assert_eq!(mgr.free_enabled_count(), 4);
}

#[test]
fn stop_frees_cores_and_reaps_child() {
    let mut mgr = ResourceManager::fake(8).unwrap();
    mgr.start_session(SessionId(1), 2, dummy_spec()).unwrap();
    mgr.stop_session(SessionId(1)).unwrap();
    assert_eq!(mgr.session_count(), 0);
    assert_eq!(mgr.free_enabled_count(), 6);
    assert!(matches!(
        mgr.session_alive(SessionId(1)),
        Err(multiplexer_resman::ManagerError::UnknownSession(1))
    ));
}

#[test]
fn drop_manager_reaps_all_sessions() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use multiplexer_resman::FakeWatch;

    let watches: Rc<RefCell<Vec<FakeWatch>>> = Rc::new(RefCell::new(Vec::new()));
    let captured = watches.clone();
    let mut mgr = ResourceManager::new(8, move || {
        let c = FakeContainment::new();
        captured.borrow_mut().push(c.watch());
        c
    })
    .unwrap();
    mgr.start_session(SessionId(1), 1, dummy_spec()).unwrap();
    mgr.start_session(SessionId(2), 1, dummy_spec()).unwrap();
    let child1 = mgr.child_id(SessionId(1)).unwrap();
    let child2 = mgr.child_id(SessionId(2)).unwrap();
    drop(mgr);
    let watches = watches.borrow();
    assert!(!watches[0].child_alive(child1).unwrap());
    assert!(!watches[1].child_alive(child2).unwrap());
}

#[test]
fn insufficient_cores_does_not_leave_a_session() {
    let mut mgr = ResourceManager::fake(4).unwrap();
    let err = mgr
        .start_session(SessionId(1), 8, dummy_spec())
        .unwrap_err();
    assert!(matches!(err, multiplexer_resman::ManagerError::Bitmap(_)));
    assert_eq!(mgr.session_count(), 0);
}

#[test]
fn stop_unknown_session_errors() {
    let mut mgr = ResourceManager::fake(8).unwrap();
    assert!(matches!(
        mgr.stop_session(SessionId(9)),
        Err(multiplexer_resman::ManagerError::UnknownSession(9))
    ));
}

#[test]
fn double_start_same_session_errors() {
    let mut mgr = ResourceManager::fake(8).unwrap();
    mgr.start_session(SessionId(1), 1, dummy_spec()).unwrap();
    assert!(mgr.start_session(SessionId(1), 1, dummy_spec()).is_err());
}

/// Containment that fails the first spawn.
struct FailOnce {
    inner: FakeContainment,
}

impl multiplexer_resman::Containment for FailOnce {
    fn spawn(
        &mut self,
        _spec: SpawnSpec,
    ) -> Result<multiplexer_resman::ContainedChild, ContainmentError> {
        Err(ContainmentError::Spawn {
            program: "x".into(),
            message: "boom".into(),
        })
    }

    fn child_alive(&self, id: multiplexer_resman::ChildId) -> Result<bool, ContainmentError> {
        self.inner.child_alive(id)
    }
}

#[test]
fn failed_spawn_does_not_consume_cores() {
    let mut mgr = ResourceManager::new(8, || FailOnce {
        inner: FakeContainment::new(),
    })
    .unwrap();
    let before = mgr.free_enabled_count();
    assert!(mgr.start_session(SessionId(1), 2, dummy_spec()).is_err());
    assert_eq!(mgr.free_enabled_count(), before);
    assert_eq!(mgr.session_count(), 0);
}

proptest::proptest! {
    #[test]
    fn start_then_stop_restores_free_cores(cores in 1usize..5) {
        let mut mgr = ResourceManager::fake(12).unwrap();
        let free0 = mgr.free_enabled_count();
        mgr.start_session(SessionId(1), cores, dummy_spec()).unwrap();
        mgr.stop_session(SessionId(1)).unwrap();
        assert_eq!(mgr.free_enabled_count(), free0);
        assert_eq!(mgr.session_count(), 0);
    }
}
