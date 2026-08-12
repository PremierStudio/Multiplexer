//! Unit and property tests for the in-memory terminal hub.

use std::collections::HashSet;
use std::path::PathBuf;

use multiplexer_terminal::{
    TerminalError, TerminalHub, TerminalId, TerminalSnapshot, TerminalSpec,
};
use proptest::prelude::*;

fn spec(cols: u16, rows: u16) -> TerminalSpec {
    TerminalSpec::new(cols, rows, "/work")
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn types_are_send_sync() {
    assert_send_sync::<TerminalId>();
    assert_send_sync::<TerminalSpec>();
    assert_send_sync::<TerminalSnapshot>();
    assert_send_sync::<TerminalError>();
    assert_send_sync::<TerminalHub>();
}

#[test]
fn empty_hub_lists_nothing() {
    let hub = TerminalHub::new();
    assert!(hub.list().is_empty());
    assert_eq!(hub.get(&TerminalId::from("term-1")), None);
    assert!(!hub.is_alive(&TerminalId::from("term-1")));
    assert_eq!(hub.input_buffer(&TerminalId::from("term-1")), None);
}

#[test]
fn default_matches_new() {
    let mut a = TerminalHub::default();
    let mut b = TerminalHub::new();
    let ia = a.create(spec(80, 24));
    let ib = b.create(spec(80, 24));
    assert_eq!(ia.0, "term-1");
    assert_eq!(ib.0, "term-1");
    assert_eq!(ia.as_str(), "term-1");
    assert_eq!(ia.to_string(), "term-1");
}

#[test]
fn create_assigns_global_term_ids() {
    let mut hub = TerminalHub::new();
    let a = hub.create(spec(80, 24));
    let b = hub.create(spec(100, 30));
    let c = hub.create(spec(40, 12));
    assert_eq!(a.0, "term-1");
    assert_eq!(b.0, "term-2");
    assert_eq!(c.0, "term-3");
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_eq!(hub.list(), vec![a, b, c]);
}

#[test]
fn create_records_spec_and_starts_alive() {
    let mut hub = TerminalHub::new();
    let id = hub.create(TerminalSpec::new(132, 43, "C:\\proj"));
    let snap = hub.get(&id).expect("created");
    assert_eq!(snap.id, id);
    assert_eq!(snap.spec.cols, 132);
    assert_eq!(snap.spec.rows, 43);
    assert_eq!(snap.spec.cwd, PathBuf::from("C:\\proj"));
    assert!(snap.alive);
    assert!(snap.input.is_empty());
    assert!(hub.is_alive(&id));
}

#[test]
fn list_is_creation_order_and_live_only() {
    let mut hub = TerminalHub::new();
    let a = hub.create(spec(80, 24));
    let b = hub.create(spec(80, 24));
    let c = hub.create(spec(80, 24));
    hub.kill(&b).expect("kill b");
    assert_eq!(hub.list(), vec![a.clone(), c.clone()]);
    assert!(!hub.list().iter().any(|id| id == &b));
    assert_eq!(hub.list().len(), 2);
}

#[test]
fn resize_updates_cols_and_rows_independently() {
    let mut hub = TerminalHub::new();
    let id = hub.create(spec(80, 24));
    hub.resize(&id, 120, 24).expect("cols");
    let after_cols = hub.get(&id).expect("snap");
    assert_eq!(after_cols.spec.cols, 120);
    assert_eq!(after_cols.spec.rows, 24);
    hub.resize(&id, 120, 40).expect("rows");
    let after_rows = hub.get(&id).expect("snap");
    assert_eq!(after_rows.spec.cols, 120);
    assert_eq!(after_rows.spec.rows, 40);
    assert_eq!(after_rows.spec.cwd, PathBuf::from("/work"));
}

#[test]
fn input_appends_to_buffer() {
    let mut hub = TerminalHub::new();
    let id = hub.create(spec(80, 24));
    hub.input(&id, b"ab").expect("first");
    hub.input(&id, b"cd").expect("second");
    hub.input(&id, b"").expect("empty");
    assert_eq!(hub.input_buffer(&id).as_deref(), Some(&b"abcd"[..]));
    let snap = hub.get(&id).expect("snap");
    assert_eq!(snap.input, b"abcd");
    assert_ne!(snap.input, b"ab");
    assert_ne!(snap.input, b"cd");
}

#[test]
fn kill_marks_dead_and_rejects_further_commands() {
    let mut hub = TerminalHub::new();
    let id = hub.create(spec(80, 24));
    hub.input(&id, b"keep").expect("buffer before kill");
    hub.kill(&id).expect("kill");
    assert!(!hub.is_alive(&id));
    assert!(hub.list().is_empty());
    let snap = hub.get(&id).expect("still snapshotable");
    assert!(!snap.alive);
    assert_eq!(snap.input, b"keep");
    assert_eq!(hub.input_buffer(&id).as_deref(), Some(&b"keep"[..]));
    assert_eq!(
        hub.kill(&id).expect_err("already dead"),
        TerminalError::NotFound(id.clone())
    );
    assert_eq!(
        hub.resize(&id, 10, 10).expect_err("dead"),
        TerminalError::NotFound(id.clone())
    );
    assert_eq!(
        hub.input(&id, b"x").expect_err("dead"),
        TerminalError::NotFound(id.clone())
    );
}

#[test]
fn unknown_id_is_not_found() {
    let mut hub = TerminalHub::new();
    let missing = TerminalId::from("term-99");
    assert_eq!(
        hub.resize(&missing, 80, 24).unwrap_err(),
        TerminalError::NotFound(missing.clone())
    );
    assert_eq!(
        hub.input(&missing, b"x").unwrap_err(),
        TerminalError::NotFound(missing.clone())
    );
    assert_eq!(
        hub.kill(&missing).unwrap_err(),
        TerminalError::NotFound(missing.clone())
    );
    assert_eq!(
        hub.kill(&missing).unwrap_err().to_string(),
        "not found: term-99"
    );
    assert!(!hub.is_alive(&missing));
    assert_eq!(hub.get(&missing), None);
}

#[test]
fn drop_hub_kills_all() {
    let mut hub = TerminalHub::new();
    let a = hub.create(spec(80, 24));
    let b = hub.create(spec(40, 12));
    let watch = hub.watch();
    assert!(watch.is_alive(&a));
    assert!(watch.is_alive(&b));
    assert_eq!(watch.list().len(), 2);
    drop(hub);
    assert!(!watch.is_alive(&a));
    assert!(!watch.is_alive(&b));
    assert!(watch.list().is_empty());
    let snap_a = watch.get(&a).expect("retained");
    let snap_b = watch.get(&b).expect("retained");
    assert!(!snap_a.alive);
    assert!(!snap_b.alive);
    assert_eq!(snap_a.spec.cols, 80);
    assert_eq!(snap_b.spec.rows, 12);
}

#[test]
fn watch_does_not_kill_on_drop() {
    let mut hub = TerminalHub::new();
    let id = hub.create(spec(80, 24));
    let watch = hub.watch();
    drop(watch);
    assert!(hub.is_alive(&id));
    assert_eq!(hub.list(), vec![id]);
}

#[test]
fn independent_hubs_do_not_share_counters() {
    let mut a = TerminalHub::new();
    let mut b = TerminalHub::new();
    let _ = a.create(spec(80, 24));
    let _ = a.create(spec(80, 24));
    let first_b = b.create(spec(80, 24));
    assert_eq!(first_b.0, "term-1");
}

#[test]
fn resize_unknown_does_not_create() {
    let mut hub = TerminalHub::new();
    let _ = hub.resize(&TerminalId::from("term-1"), 1, 1);
    assert!(hub.list().is_empty());
}

proptest! {
    #[test]
    fn create_n_unique_sequential_ids(n in 1usize..=40) {
        let mut hub = TerminalHub::new();
        let mut seen = HashSet::new();
        for i in 0..n {
            let id = hub.create(spec(80, 24));
            prop_assert_eq!(id.as_str(), format!("term-{}", i + 1));
            prop_assert!(seen.insert(id.0.clone()), "duplicate id {}", id);
        }
        let listed = hub.list();
        prop_assert_eq!(listed.len(), n);
        let list_ids: HashSet<String> = listed.iter().map(|id| id.0.clone()).collect();
        prop_assert_eq!(list_ids.len(), n);
        prop_assert_eq!(list_ids, seen);
        for (i, id) in listed.iter().enumerate() {
            prop_assert_eq!(&id.0, &format!("term-{}", i + 1));
        }
    }

    #[test]
    fn input_concatenates_chunks(chunks in prop::collection::vec(prop::collection::vec(any::<u8>(), 0..8), 1..8)) {
        let mut hub = TerminalHub::new();
        let id = hub.create(spec(80, 24));
        let mut expected = Vec::new();
        for chunk in &chunks {
            hub.input(&id, chunk).expect("input");
            expected.extend_from_slice(chunk);
        }
        prop_assert_eq!(hub.input_buffer(&id).unwrap(), expected);
    }

    #[test]
    fn resize_round_trip(cols in 1u16..=400, rows in 1u16..=200) {
        let mut hub = TerminalHub::new();
        let id = hub.create(spec(80, 24));
        hub.resize(&id, cols, rows).expect("resize");
        let snap = hub.get(&id).expect("snap");
        prop_assert_eq!(snap.spec.cols, cols);
        prop_assert_eq!(snap.spec.rows, rows);
    }
}
