use multiplexer_resman::{ChildId, Containment, ContainmentError, FakeContainment, SpawnSpec};
use proptest::prelude::*;
use std::path::PathBuf;

fn spec() -> SpawnSpec {
    SpawnSpec {
        program: PathBuf::from("never-runs"),
        args: vec!["--fake".into()],
        memory_cap_bytes: None,
    }
}

#[test]
fn spawn_alive_then_drop_not_alive() {
    let mut containment = FakeContainment::new();
    let watch = containment.watch();
    let child = containment.spawn(spec()).expect("fake spawn");
    assert!(
        containment.child_alive(child.id).expect("known child"),
        "child must be alive after spawn"
    );
    assert_eq!(child.pid, 0, "fake never starts an OS process");

    drop(containment);
    assert!(
        !watch.child_alive(child.id).expect("known child"),
        "drop must mark the child dead"
    );
}

#[test]
fn two_children_both_die_on_drop() {
    let mut containment = FakeContainment::new();
    let watch = containment.watch();
    let first = containment.spawn(spec()).expect("first spawn");
    let second = containment.spawn(spec()).expect("second spawn");
    assert_eq!(first.id, ChildId(1));
    assert_eq!(second.id, ChildId(2));
    assert!(containment.child_alive(first.id).expect("first known"));
    assert!(containment.child_alive(second.id).expect("second known"));

    drop(containment);
    assert!(!watch.child_alive(first.id).expect("first known"));
    assert!(!watch.child_alive(second.id).expect("second known"));
}

#[test]
fn child_alive_unknown_id_errors() {
    let containment = FakeContainment::new();
    assert_eq!(
        containment.child_alive(ChildId(99)).err(),
        Some(ContainmentError::UnknownChild(99))
    );
}

#[test]
fn close_reaps_the_tree() {
    let mut containment = FakeContainment::new();
    let watch = containment.watch();
    let child = containment.spawn(spec()).expect("fake spawn");
    containment.close();
    assert!(!watch.child_alive(child.id).expect("known child"));
}

#[test]
fn watch_unknown_id_errors() {
    let containment = FakeContainment::new();
    let watch = containment.watch();
    assert_eq!(
        watch.child_alive(ChildId(1)).err(),
        Some(ContainmentError::UnknownChild(1))
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn drop_reaps_every_spawned_child(n in 0usize..16) {
        let mut containment = FakeContainment::new();
        let watch = containment.watch();
        let mut ids = Vec::with_capacity(n);
        for _ in 0..n {
            ids.push(containment.spawn(spec()).expect("fake spawn").id);
        }
        drop(containment);
        for id in ids {
            prop_assert!(!watch.child_alive(id).expect("known child"));
        }
    }
}
