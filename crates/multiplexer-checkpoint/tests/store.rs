//! Unit and property tests for the in-memory checkpoint store.

use std::collections::HashSet;

use multiplexer_checkpoint::{Checkpoint, CheckpointError, CheckpointId, CheckpointStore};
use proptest::prelude::*;

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn types_are_send_sync() {
    assert_send_sync::<CheckpointId>();
    assert_send_sync::<Checkpoint>();
    assert_send_sync::<CheckpointStore>();
    assert_send_sync::<CheckpointError>();
}

#[test]
fn empty_store_has_no_current_and_empty_list() {
    let store = CheckpointStore::new();
    assert!(store.list("s").is_empty());
    assert_eq!(store.current("s"), None);
    assert_eq!(store.get(&CheckpointId::from("cp-1")), None);
}

#[test]
fn default_matches_new() {
    let mut a = CheckpointStore::default();
    let mut b = CheckpointStore::new();
    let ca = a.create("s", "first");
    let cb = b.create("s", "first");
    assert_eq!(ca, cb);
    assert_eq!(ca.id.0, "cp-1");
}

#[test]
fn create_assigns_global_cp_ids() {
    let mut store = CheckpointStore::new();
    let a = store.create("alpha", "one");
    let b = store.create("beta", "two");
    let c = store.create("alpha", "three");
    assert_eq!(a.id.0, "cp-1");
    assert_eq!(b.id.0, "cp-2");
    assert_eq!(c.id.0, "cp-3");
    assert_eq!(a.id.as_str(), "cp-1");
    assert_eq!(a.id.to_string(), "cp-1");
    assert_ne!(a.id, b.id);
    assert_ne!(b.id, c.id);
}

#[test]
fn create_records_session_label_and_per_session_seq() {
    let mut store = CheckpointStore::new();
    let a = store.create("alpha", "pre");
    let b = store.create("beta", "other");
    let c = store.create("alpha", "post");
    assert_eq!(a.session_id, "alpha");
    assert_eq!(a.label, "pre");
    assert_eq!(a.seq, 1);
    assert_eq!(b.session_id, "beta");
    assert_eq!(b.label, "other");
    assert_eq!(b.seq, 1);
    assert_eq!(c.session_id, "alpha");
    assert_eq!(c.label, "post");
    assert_eq!(c.seq, 2);
}

#[test]
fn create_sets_current_to_new_id() {
    let mut store = CheckpointStore::new();
    let a = store.create("s", "a");
    assert_eq!(store.current("s"), Some(a.id.clone()));
    let b = store.create("s", "b");
    assert_eq!(store.current("s"), Some(b.id.clone()));
    assert_ne!(store.current("s"), Some(a.id));
}

#[test]
fn list_is_chronological_and_session_scoped() {
    let mut store = CheckpointStore::new();
    let a = store.create("alpha", "a");
    let b = store.create("beta", "b");
    let c = store.create("alpha", "c");
    let alpha = store.list("alpha");
    let beta = store.list("beta");
    assert_eq!(alpha.len(), 2);
    assert_eq!(alpha[0], a);
    assert_eq!(alpha[1], c);
    assert_eq!(alpha[0].id.0, "cp-1");
    assert_eq!(alpha[1].id.0, "cp-3");
    assert_eq!(beta, vec![b]);
    assert!(store.list("missing").is_empty());
    assert_eq!(alpha[0].seq, 1);
    assert_eq!(alpha[1].seq, 2);
}

#[test]
fn get_returns_created_checkpoint() {
    let mut store = CheckpointStore::new();
    let a = store.create("s", "alpha");
    let b = store.create("s", "beta");
    assert_eq!(store.get(&a.id), Some(a.clone()));
    assert_eq!(store.get(&b.id), Some(b.clone()));
    assert_eq!(store.get(&CheckpointId::from("cp-1")), Some(a));
    assert_eq!(store.get(&CheckpointId::from("cp-2")), Some(b));
    assert_eq!(store.get(&CheckpointId::from("cp-3")), None);
    assert_eq!(store.get(&CheckpointId::from("cp-0")), None);
    assert_eq!(store.get(&CheckpointId::from("")), None);
}

#[test]
fn revert_unknown_is_not_found_and_does_not_move_current() {
    let mut store = CheckpointStore::new();
    let created = store.create("s", "keep");
    let before = store.current("s");
    let missing = CheckpointId::from("cp-99");
    let err = store.revert(&missing).expect_err("unknown id");
    assert_eq!(err, CheckpointError::NotFound(missing.clone()));
    assert_eq!(err.to_string(), "not found: cp-99");
    assert_eq!(store.current("s"), before);
    assert_eq!(store.current("s"), Some(created.id.clone()));
    assert_eq!(store.list("s").len(), 1);
    assert_eq!(store.get(&created.id), Some(created));
}

#[test]
fn revert_sets_current_to_that_id() {
    let mut store = CheckpointStore::new();
    let a = store.create("s", "a");
    let b = store.create("s", "b");
    assert_eq!(store.current("s"), Some(b.id.clone()));
    let reverted = store.revert(&a.id).expect("revert to a");
    assert_eq!(reverted, a);
    assert_eq!(store.current("s"), Some(a.id.clone()));
    assert_eq!(
        store.current("s").as_ref().map(|id| id.as_str()),
        Some("cp-1")
    );
    let again = store.revert(&b.id).expect("revert to b");
    assert_eq!(again, b);
    assert_eq!(store.current("s"), Some(b.id));
}

#[test]
fn revert_is_non_destructive() {
    let mut store = CheckpointStore::new();
    let a = store.create("s", "a");
    let b = store.create("s", "b");
    store.revert(&a.id).expect("revert");
    let listed = store.list("s");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0], a);
    assert_eq!(listed[1], b);
    assert_eq!(store.get(&b.id), Some(b));
}

#[test]
fn revert_only_moves_own_session_pointer() {
    let mut store = CheckpointStore::new();
    let a1 = store.create("alpha", "a1");
    let b1 = store.create("beta", "b1");
    let a2 = store.create("alpha", "a2");
    store.revert(&a1.id).expect("revert alpha");
    assert_eq!(store.current("alpha"), Some(a1.id.clone()));
    assert_eq!(store.current("beta"), Some(b1.id.clone()));
    assert_eq!(store.list("alpha"), vec![a1, a2]);
}

#[test]
fn create_after_revert_continues_global_ids_and_session_seq() {
    let mut store = CheckpointStore::new();
    let a = store.create("s", "a");
    let _b = store.create("s", "b");
    store.revert(&a.id).expect("revert");
    let c = store.create("s", "c");
    assert_eq!(c.id.0, "cp-3");
    assert_eq!(c.seq, 3);
    assert_eq!(store.current("s"), Some(c.id.clone()));
    assert_eq!(store.list("s").len(), 3);
}

#[test]
fn empty_label_and_session_are_stored() {
    let mut store = CheckpointStore::new();
    let cp = store.create("", "");
    assert_eq!(cp.session_id, "");
    assert_eq!(cp.label, "");
    assert_eq!(cp.seq, 1);
    assert_eq!(store.list(""), vec![cp.clone()]);
    assert_eq!(store.current(""), Some(cp.id));
}

#[test]
fn independent_stores_do_not_share_counters() {
    let mut a = CheckpointStore::new();
    let mut b = CheckpointStore::new();
    let _ = a.create("s", "x");
    let _ = a.create("s", "y");
    let first_b = b.create("s", "z");
    assert_eq!(first_b.id.0, "cp-1");
    assert_eq!(first_b.seq, 1);
}

#[test]
fn checkpoint_id_from_and_error_display() {
    let id = CheckpointId::from(String::from("cp-7"));
    assert_eq!(id.as_str(), "cp-7");
    assert_eq!(CheckpointError::NotFound(id).to_string(), "not found: cp-7");
}

proptest! {
    #[test]
    fn n_creates_one_session_list_len_and_unique_ids(n in 1usize..=40) {
        let mut store = CheckpointStore::new();
        let mut seen = HashSet::new();
        for i in 0..n {
            let cp = store.create("sess", &format!("label-{i}"));
            prop_assert_eq!(cp.seq, (i as u64) + 1);
            prop_assert_eq!(cp.id.as_str(), &format!("cp-{}", i + 1));
            prop_assert!(seen.insert(cp.id.0.clone()), "duplicate id {}", cp.id);
            let current = store.current("sess");
            prop_assert_eq!(current.as_ref(), Some(&cp.id));
        }
        let listed = store.list("sess");
        prop_assert_eq!(listed.len(), n);
        let list_ids: HashSet<String> = listed.iter().map(|c| c.id.0.clone()).collect();
        prop_assert_eq!(list_ids.len(), n);
        prop_assert_eq!(list_ids, seen);
        for (i, cp) in listed.iter().enumerate() {
            prop_assert_eq!(cp.seq, (i as u64) + 1);
            prop_assert_eq!(&cp.id.0, &format!("cp-{}", i + 1));
            prop_assert_eq!(&cp.session_id, "sess");
        }
    }

    #[test]
    fn global_ids_unique_across_two_sessions(n in 1usize..=16, m in 1usize..=16) {
        let mut store = CheckpointStore::new();
        let mut seen = HashSet::new();
        for i in 0..n {
            let cp = store.create("a", &format!("a{i}"));
            prop_assert!(seen.insert(cp.id.0.clone()));
        }
        for i in 0..m {
            let cp = store.create("b", &format!("b{i}"));
            prop_assert!(seen.insert(cp.id.0.clone()));
        }
        prop_assert_eq!(store.list("a").len(), n);
        prop_assert_eq!(store.list("b").len(), m);
        prop_assert_eq!(seen.len(), n + m);
        prop_assert_eq!(store.list("a").last().map(|c| c.seq), Some(n as u64));
        prop_assert_eq!(store.list("b").last().map(|c| c.seq), Some(m as u64));
    }

    #[test]
    fn revert_unknown_never_succeeds(id in "[a-z0-9-]{1,12}") {
        let mut store = CheckpointStore::new();
        let created = store.create("s", "keep");
        prop_assume!(id != created.id.0);
        let err = store
            .revert(&CheckpointId::from(id.as_str()))
            .expect_err("must be missing");
        prop_assert_eq!(err, CheckpointError::NotFound(CheckpointId::from(id.as_str())));
        prop_assert_eq!(store.current("s"), Some(created.id));
    }
}
