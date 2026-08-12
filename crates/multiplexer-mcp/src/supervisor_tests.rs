use super::*;

fn cfg(name: &str, command: &str) -> ServerConfig {
    ServerConfig::new(name, command, Vec::new(), Vec::new())
}

#[test]
fn release_unknown_token_errors() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("x", "npx"));
    let forged = ServerHandle {
        id: handle.id.clone(),
        hash: handle.hash,
        token: 999,
    };
    assert_eq!(
        sup.release(forged),
        Err(SupervisorError::UnknownHandle("x".into()))
    );
    assert_eq!(sup.refcount(&handle.id), Some(1));
    sup.release(handle).unwrap();
    assert_eq!(sup.instance_count(), 0);
}

#[test]
fn release_missing_instance_errors() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("x", "npx"));
    sup.instances.clear();
    assert_eq!(
        sup.release(handle),
        Err(SupervisorError::UnknownHandle("x".into()))
    );
}

#[test]
fn release_ok_when_name_index_missing() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("x", "npx"));
    sup.names.clear();
    assert!(sup.release(handle).is_ok());
    assert_eq!(sup.instance_count(), 0);
}

#[test]
fn lookup_skips_hashes_without_instance() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("x", "npx"));
    let id = handle.id.clone();
    let stale = config_hash(&cfg("x", "other"));
    sup.names.get_mut(&id).unwrap().insert(0, stale);
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.refcount(&id), Some(1));
    assert_eq!(sup.restart_count(&id), Some(0));
    assert_eq!(sup.backoff_ms(&id), None);
}

#[test]
fn mark_crashed_stopped_is_illegal() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("x", "npx"));
    let hash = handle.hash;
    sup.instances.get_mut(&hash).unwrap().state = LifecycleState::Stopped;
    assert_eq!(
        sup.mark_crashed_hash(&hash),
        Err(SupervisorError::IllegalTransition {
            from: LifecycleState::Stopped
        })
    );
}

#[test]
fn mark_crashed_from_spawned_and_crashed_states() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("x", "npx"));
    let hash = handle.hash;
    sup.instances.get_mut(&hash).unwrap().state = LifecycleState::Spawned;
    assert_eq!(sup.mark_crashed_hash(&hash).unwrap(), LifecycleState::Ready);
    sup.instances.get_mut(&hash).unwrap().state = LifecycleState::Crashed { restarts: 1 };
    assert_eq!(sup.mark_crashed_hash(&hash).unwrap(), LifecycleState::Ready);
    assert_eq!(sup.restart_count(handle.id()), Some(2));
}

#[test]
fn name_lookup_treats_stopped_like_failed() {
    let mut sup = Supervisor::new();
    let older = sup.acquire(&cfg("tools", "npx"));
    let newer = sup.acquire(&cfg("tools", "uvx"));
    let id = ServerId::new("tools");
    sup.instances.get_mut(&older.hash).unwrap().state = LifecycleState::Stopped;
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.state_hash(&newer.hash), Some(LifecycleState::Ready));
    assert_eq!(sup.refcount(&id), Some(1));
}

#[test]
fn default_supervisor_is_empty() {
    assert_eq!(Supervisor::default().instance_count(), 0);
}
