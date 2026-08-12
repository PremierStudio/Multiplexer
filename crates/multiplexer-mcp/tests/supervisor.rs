//! Unit and property tests for the MCP lifecycle supervisor.

use multiplexer_mcp::{
    backoff_ms_for, config_hash, ConfigHash, LifecycleState, ServerConfig, ServerId, Supervisor,
    SupervisorError, BACKOFF_BASE_MS, BACKOFF_CAP_MS, MAX_CONSECUTIVE_FAILURES,
};
use proptest::prelude::*;

fn cfg(name: &str, command: &str) -> ServerConfig {
    ServerConfig::new(name, command, Vec::new(), Vec::new())
}

fn cfg_full(name: &str, command: &str, args: &[&str], env_keys: &[&str]) -> ServerConfig {
    ServerConfig::new(
        name,
        command,
        args.iter().map(|s| (*s).to_string()).collect(),
        env_keys.iter().map(|s| (*s).to_string()).collect(),
    )
}

#[test]
fn config_hash_stable_for_same_fields() {
    let a = cfg_full("linear", "npx", &["-y", "mcp-linear"], &["LINEAR_API_KEY"]);
    let b = cfg_full("linear", "npx", &["-y", "mcp-linear"], &["LINEAR_API_KEY"]);
    assert_eq!(config_hash(&a), config_hash(&b));
    assert_eq!(config_hash(&a).as_bytes().len(), 32);
    assert_eq!(config_hash(&a).to_hex().len(), 64);
}

#[test]
fn config_hash_fixture_and_hex_round_trip() {
    let cfg = cfg_full("linear", "npx", &["-y", "mcp-linear"], &["LINEAR_API_KEY"]);
    let hash = config_hash(&cfg);
    let bytes = hash.as_bytes();
    assert_ne!(*bytes, [0u8; 32]);
    assert_ne!(*bytes, [1u8; 32]);
    let mut encoded = String::with_capacity(64);
    for byte in bytes {
        encoded.push_str(&format!("{byte:02x}"));
    }
    assert_eq!(hash.to_hex(), encoded);
    assert_eq!(hash.to_string(), encoded);
    assert_eq!(format!("{hash:?}"), format!("ConfigHash(\"{encoded}\")"));
    assert_eq!(
        encoded,
        "b69f9160f4e472fec3f0d789813eb500a8060b09cbd08a65666c12ba18de4b45"
    );
}

#[test]
fn config_hash_env_keys_order_independent() {
    let a = cfg_full("s", "cmd", &[], &["B", "A"]);
    let b = cfg_full("s", "cmd", &[], &["A", "B"]);
    assert_eq!(config_hash(&a), config_hash(&b));
}

#[test]
fn config_hash_args_order_matters() {
    let a = cfg_full("s", "cmd", &["x", "y"], &[]);
    let b = cfg_full("s", "cmd", &["y", "x"], &[]);
    assert_ne!(config_hash(&a), config_hash(&b));
}

#[test]
fn config_hash_field_changes_differ() {
    let base = cfg_full("s", "cmd", &["a"], &["K"]);
    assert_ne!(
        config_hash(&base),
        config_hash(&cfg_full("t", "cmd", &["a"], &["K"]))
    );
    assert_ne!(
        config_hash(&base),
        config_hash(&cfg_full("s", "other", &["a"], &["K"]))
    );
    assert_ne!(
        config_hash(&base),
        config_hash(&cfg_full("s", "cmd", &["b"], &["K"]))
    );
    assert_ne!(
        config_hash(&base),
        config_hash(&cfg_full("s", "cmd", &["a"], &["Z"]))
    );
}

#[test]
fn config_hash_length_prefix_avoids_boundary_collision() {
    let a = cfg_full("ab", "c", &[], &[]);
    let b = cfg_full("a", "bc", &[], &[]);
    assert_ne!(config_hash(&a), config_hash(&b));
}

#[test]
fn backoff_schedule_matches_plan() {
    assert_eq!(MAX_CONSECUTIVE_FAILURES, 5);
    assert_eq!(BACKOFF_BASE_MS, 1_000);
    assert_eq!(BACKOFF_CAP_MS, 30_000);
    assert_eq!(backoff_ms_for(0), 1_000);
    assert_eq!(backoff_ms_for(1), 2_000);
    assert_eq!(backoff_ms_for(2), 4_000);
    assert_eq!(backoff_ms_for(3), 8_000);
    assert_eq!(backoff_ms_for(4), 16_000);
    assert_eq!(backoff_ms_for(5), 30_000);
    assert_eq!(backoff_ms_for(31), 30_000);
}

#[test]
fn acquire_starts_ready_with_refcount_one() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("ctx7", "npx"));
    let id = handle.id().clone();
    assert_eq!(id, ServerId::new("ctx7"));
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.refcount(&id), Some(1));
    assert_eq!(sup.instance_count(), 1);
    assert_eq!(sup.restart_count(&id), Some(0));
    assert_eq!(sup.backoff_ms(&id), None);
    assert_eq!(handle.hash(), config_hash(&cfg("ctx7", "npx")));
}

#[test]
fn same_config_hash_reuses_one_instance() {
    let mut sup = Supervisor::new();
    let a = cfg("linear", "npx");
    let h1 = sup.acquire(&a);
    let h2 = sup.acquire(&a);
    let id = ServerId::new("linear");
    assert_eq!(h1.hash(), h2.hash());
    assert_eq!(sup.instance_count(), 1);
    assert_eq!(sup.refcount(&id), Some(2));
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    sup.release(h1).unwrap();
    assert_eq!(sup.instance_count(), 1);
    assert_eq!(sup.refcount(&id), Some(1));
    sup.release(h2).unwrap();
    assert_eq!(sup.instance_count(), 0);
    assert_eq!(sup.state(&id), None);
}

#[test]
fn different_hash_yields_two_instances() {
    let mut sup = Supervisor::new();
    let h1 = sup.acquire(&cfg("alpha", "npx"));
    let h2 = sup.acquire(&cfg("beta", "uvx"));
    assert_ne!(h1.hash(), h2.hash());
    assert_eq!(sup.instance_count(), 2);
    assert_eq!(sup.refcount(&ServerId::new("alpha")), Some(1));
    assert_eq!(sup.refcount(&ServerId::new("beta")), Some(1));
    sup.release(h1).unwrap();
    assert_eq!(sup.instance_count(), 1);
    sup.release(h2).unwrap();
    assert_eq!(sup.instance_count(), 0);
}

#[test]
fn release_to_zero_removes_instance() {
    let mut sup = Supervisor::new();
    let cfg = cfg("shadcn", "npx");
    let handle = sup.acquire(&cfg);
    let id = handle.id().clone();
    sup.release(handle).unwrap();
    assert_eq!(sup.instance_count(), 0);
    assert_eq!(sup.state(&id), None);
    assert_eq!(sup.refcount(&id), None);
    // Next acquire is a new instance, not a Stopped reuse.
    let again = sup.acquire(&cfg);
    assert_eq!(sup.state(again.id()), Some(LifecycleState::Ready));
    assert_eq!(sup.restart_count(again.id()), Some(0));
    assert_eq!(sup.instance_count(), 1);
}

#[test]
fn crash_backoff_then_failed_on_fifth() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("mailtrap", "npx"));
    let id = handle.id().clone();
    let expected = [1_000, 2_000, 4_000, 8_000];
    for (i, backoff) in expected.iter().enumerate() {
        let state = sup.mark_crashed(&id).unwrap();
        assert_eq!(state, LifecycleState::Ready);
        assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
        assert_eq!(sup.restart_count(&id), Some((i as u32) + 1));
        assert_eq!(sup.backoff_ms(&id), Some(*backoff));
        assert_eq!(sup.refcount(&id), Some(1));
    }
    let failed = sup.mark_crashed(&id).unwrap();
    assert_eq!(failed, LifecycleState::Failed);
    assert_eq!(sup.state(&id), Some(LifecycleState::Failed));
    assert_eq!(sup.restart_count(&id), Some(5));
    assert_eq!(sup.backoff_ms(&id), None);
    assert_eq!(sup.instance_count(), 1);
}

#[test]
fn mark_crashed_on_failed_is_illegal() {
    let mut sup = Supervisor::new();
    let handle = sup.acquire(&cfg("broken", "npx"));
    let id = handle.id().clone();
    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        let _ = sup.mark_crashed(&id).unwrap();
    }
    assert_eq!(sup.state(&id), Some(LifecycleState::Failed));
    assert_eq!(
        sup.mark_crashed(&id),
        Err(SupervisorError::IllegalTransition {
            from: LifecycleState::Failed
        })
    );
}

#[test]
fn acquire_after_failed_released_is_new_instance() {
    let mut sup = Supervisor::new();
    let cfg = cfg("remote", "mcp-remote");
    let handle = sup.acquire(&cfg);
    let id = handle.id().clone();
    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        let _ = sup.mark_crashed(&id).unwrap();
    }
    assert_eq!(sup.state(&id), Some(LifecycleState::Failed));
    sup.release(handle).unwrap();
    assert_eq!(sup.instance_count(), 0);

    let again = sup.acquire(&cfg);
    assert_eq!(sup.state(again.id()), Some(LifecycleState::Ready));
    assert_eq!(sup.restart_count(again.id()), Some(0));
    assert_eq!(sup.backoff_ms(again.id()), None);
    assert_eq!(sup.instance_count(), 1);
}

#[test]
fn mark_crashed_unknown_errors() {
    let mut sup = Supervisor::new();
    assert_eq!(
        sup.mark_crashed(&ServerId::new("missing")),
        Err(SupervisorError::UnknownServer("missing".into()))
    );
}

#[test]
fn queries_on_unknown_id_are_none() {
    let sup = Supervisor::new();
    let id = ServerId::new("nope");
    assert_eq!(sup.state(&id), None);
    assert_eq!(sup.refcount(&id), None);
    assert_eq!(sup.restart_count(&id), None);
    assert_eq!(sup.backoff_ms(&id), None);
    assert_eq!(sup.instance_count(), 0);
}

#[test]
fn same_name_different_command_is_two_hashes() {
    let mut sup = Supervisor::new();
    let h1 = sup.acquire(&cfg("tools", "npx"));
    let h2 = sup.acquire(&cfg("tools", "uvx"));
    assert_ne!(h1.hash(), h2.hash());
    assert_eq!(sup.instance_count(), 2);
    assert_eq!(sup.refcount_hash(&h1.hash()), Some(1));
    assert_eq!(sup.refcount_hash(&h2.hash()), Some(1));
    assert_eq!(sup.state_hash(&h1.hash()), Some(LifecycleState::Ready));
    let kept = h2.hash();
    let id = ServerId::new("tools");
    sup.release(h1).unwrap();
    assert_eq!(sup.instance_count(), 1);
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.refcount(&id), Some(1));
    assert_eq!(sup.state_hash(&kept), Some(LifecycleState::Ready));
    sup.release(h2).unwrap();
    assert_eq!(sup.instance_count(), 0);
    assert_eq!(sup.state(&id), None);
}

#[test]
fn never_more_instances_than_distinct_live_hashes() {
    let mut sup = Supervisor::new();
    let configs = [
        cfg("a", "npx"),
        cfg("a", "npx"),
        cfg("b", "npx"),
        cfg("c", "uvx"),
    ];
    let mut handles = Vec::new();
    for c in &configs {
        handles.push(sup.acquire(c));
    }
    let distinct: std::collections::HashSet<ConfigHash> =
        handles.iter().map(|h| h.hash()).collect();
    assert_eq!(sup.instance_count(), distinct.len());
    assert!(sup.instance_count() <= distinct.len());
}

#[test]
fn default_supervisor_is_empty() {
    let sup = Supervisor::default();
    assert_eq!(sup.instance_count(), 0);
}

#[test]
fn acquire_while_failed_resets_same_slot() {
    let mut sup = Supervisor::new();
    let cfg = cfg("broken", "npx");
    let first = sup.acquire(&cfg);
    let id = first.id().clone();
    let hash = first.hash();
    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        let _ = sup.mark_crashed(&id).unwrap();
    }
    assert_eq!(sup.state(&id), Some(LifecycleState::Failed));
    let again = sup.acquire(&cfg);
    assert_eq!(again.hash(), hash);
    assert_eq!(sup.instance_count(), 1);
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.restart_count(&id), Some(0));
    assert_eq!(sup.backoff_ms(&id), None);
    assert_eq!(sup.refcount(&id), Some(2));
    assert_eq!(sup.refcount_hash(&hash), Some(2));
}

#[test]
fn name_lookup_prefers_ready_over_failed() {
    let mut sup = Supervisor::new();
    let older = sup.acquire(&cfg("tools", "npx"));
    let id = ServerId::new("tools");
    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        let _ = sup.mark_crashed_hash(&older.hash()).unwrap();
    }
    assert_eq!(sup.state_hash(&older.hash()), Some(LifecycleState::Failed));
    let newer = sup.acquire(&cfg("tools", "uvx"));
    assert_ne!(older.hash(), newer.hash());
    assert_eq!(sup.instance_count(), 2);
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.refcount(&id), Some(1));
    assert_eq!(sup.restart_count(&id), Some(0));
    assert_eq!(sup.backoff_ms(&id), None);
    assert_eq!(sup.state_hash(&newer.hash()), Some(LifecycleState::Ready));

    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        let state = sup.mark_crashed(&id).unwrap();
        if state != LifecycleState::Failed {
            assert_eq!(state, LifecycleState::Ready);
        }
    }
    assert_eq!(sup.state_hash(&newer.hash()), Some(LifecycleState::Failed));
    assert_eq!(sup.state_hash(&older.hash()), Some(LifecycleState::Failed));
    assert_eq!(sup.state(&id), Some(LifecycleState::Failed));
}

#[test]
fn name_lookup_prefers_later_ready_instance() {
    let mut sup = Supervisor::new();
    let older = sup.acquire(&cfg("tools", "npx"));
    let newer = sup.acquire(&cfg("tools", "uvx"));
    let id = ServerId::new("tools");
    assert_eq!(sup.mark_crashed(&id).unwrap(), LifecycleState::Ready);
    assert_eq!(sup.state_hash(&older.hash()), Some(LifecycleState::Ready));
    assert_eq!(sup.state_hash(&newer.hash()), Some(LifecycleState::Ready));
    assert_eq!(sup.restart_count(&id), Some(1));
    assert_eq!(sup.backoff_ms(&id), Some(1_000));
    assert_eq!(sup.refcount_hash(&older.hash()), Some(1));
    assert_eq!(sup.refcount_hash(&newer.hash()), Some(1));
    for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
        let _ = sup.mark_crashed(&id).unwrap();
    }
    assert_eq!(sup.state_hash(&newer.hash()), Some(LifecycleState::Failed));
    assert_eq!(sup.state_hash(&older.hash()), Some(LifecycleState::Ready));
    assert_eq!(sup.state(&id), Some(LifecycleState::Ready));
    assert_eq!(sup.restart_count(&id), Some(0));
}

#[test]
fn mark_crashed_hash_unknown_errors() {
    let mut sup = Supervisor::new();
    let hash = config_hash(&cfg("missing", "npx"));
    assert_eq!(
        sup.mark_crashed_hash(&hash),
        Err(SupervisorError::UnknownServer(hash.to_hex()))
    );
    assert_eq!(sup.refcount_hash(&hash), None);
    assert_eq!(sup.state_hash(&hash), None);
}

#[test]
fn server_id_display_and_from() {
    let id = ServerId::from("linear");
    assert_eq!(id.as_str(), "linear");
    assert_eq!(id.as_ref(), "linear");
    assert_eq!(id.to_string(), "linear");
    assert_eq!(ServerId::from(String::from("linear")), id);
}

fn arb_config() -> impl Strategy<Value = ServerConfig> {
    (
        "[a-z]{1,6}",
        "[a-z]{1,6}",
        prop::collection::vec("[a-z]{0,6}", 0..3),
        prop::collection::vec("[A-Z]{0,6}", 0..3),
    )
        .prop_map(|(name, command, args, env_keys)| ServerConfig {
            name,
            command,
            args,
            env_keys,
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn identical_configs_hash_equal(cfg in arb_config()) {
        let clone = cfg.clone();
        prop_assert_eq!(config_hash(&cfg), config_hash(&clone));
    }

    #[test]
    fn env_key_permutation_same_hash(cfg in arb_config()) {
        let mut shuffled = cfg.clone();
        shuffled.env_keys.reverse();
        prop_assert_eq!(config_hash(&cfg), config_hash(&shuffled));
    }

    #[test]
    fn differing_name_hashes_differ(a in arb_config(), b in "[a-z]{1,6}") {
        prop_assume!(a.name != b);
        let mut other = a.clone();
        other.name = b;
        prop_assert_ne!(config_hash(&a), config_hash(&other));
    }

    #[test]
    fn same_hash_reuses_across_acquires(cfg in arb_config()) {
        let mut sup = Supervisor::new();
        let h1 = sup.acquire(&cfg);
        let h2 = sup.acquire(&cfg);
        prop_assert_eq!(h1.hash(), h2.hash());
        prop_assert_eq!(sup.instance_count(), 1);
        prop_assert_eq!(sup.refcount(h1.id()), Some(2));
        sup.release(h1).unwrap();
        prop_assert_eq!(sup.instance_count(), 1);
        sup.release(h2).unwrap();
        prop_assert_eq!(sup.instance_count(), 0);
    }

    #[test]
    fn two_distinct_hashes_two_instances(a in arb_config(), b in arb_config()) {
        prop_assume!(config_hash(&a) != config_hash(&b));
        let mut sup = Supervisor::new();
        let h1 = sup.acquire(&a);
        let h2 = sup.acquire(&b);
        prop_assert_eq!(sup.instance_count(), 2);
        prop_assert_ne!(h1.hash(), h2.hash());
        sup.release(h1).unwrap();
        sup.release(h2).unwrap();
        prop_assert_eq!(sup.instance_count(), 0);
    }

    #[test]
    fn instance_count_never_exceeds_distinct_live_hashes(
        configs in prop::collection::vec(arb_config(), 1..8),
        reuse in prop::collection::vec(0usize..8, 0..16),
    ) {
        let mut sup = Supervisor::new();
        let mut handles = Vec::new();
        for cfg in &configs {
            handles.push(sup.acquire(cfg));
        }
        for idx in reuse {
            if let Some(cfg) = configs.get(idx % configs.len()) {
                handles.push(sup.acquire(cfg));
            }
        }
        let distinct: std::collections::HashSet<ConfigHash> =
            handles.iter().map(|h| h.hash()).collect();
        prop_assert!(
            sup.instance_count() <= distinct.len(),
            "instances {} > distinct hashes {}",
            sup.instance_count(),
            distinct.len()
        );
        prop_assert_eq!(sup.instance_count(), distinct.len());
    }

    #[test]
    fn release_all_clears_table(configs in prop::collection::vec(arb_config(), 0..6)) {
        let mut sup = Supervisor::new();
        let mut handles = Vec::new();
        for cfg in &configs {
            handles.push(sup.acquire(cfg));
        }
        for handle in handles {
            sup.release(handle).unwrap();
        }
        prop_assert_eq!(sup.instance_count(), 0);
    }
}
