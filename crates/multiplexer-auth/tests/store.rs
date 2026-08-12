//! Unit and property tests for [`MemoryAuthStore`].

use multiplexer_auth::{AuthError, AuthStore, MemoryAuthStore, SecretRef};
use proptest::prelude::*;

fn assert_send_sync<T: Send + Sync>() {}

fn put_via_trait<S: AuthStore>(
    store: &mut S,
    name: &str,
    value: SecretRef,
) -> Result<(), AuthError> {
    store.put(name, value)
}

#[test]
fn types_are_send_sync() {
    assert_send_sync::<SecretRef>();
    assert_send_sync::<AuthError>();
    assert_send_sync::<MemoryAuthStore>();
}

#[test]
fn empty_store() {
    let store = MemoryAuthStore::new();
    assert!(store.is_empty());
    assert_eq!(store.len(), 0);
    assert_eq!(store.get("missing"), None);
}

#[test]
fn default_matches_new() {
    let mut a = MemoryAuthStore::default();
    let mut b = MemoryAuthStore::new();
    a.put("n", SecretRef::Keychain("k".into())).unwrap();
    b.put("n", SecretRef::Keychain("k".into())).unwrap();
    assert_eq!(a.get("n"), b.get("n"));
    assert_eq!(a.len(), 1);
    assert!(!a.is_empty());
}

#[test]
fn put_get_round_trip_each_variant() {
    let mut store = MemoryAuthStore::new();
    let op = SecretRef::Op("op://Vault/Item/field".into());
    let env = SecretRef::Env("${API_KEY}".into());
    let key = SecretRef::Keychain("grok".into());
    put_via_trait(&mut store, "op", op.clone()).unwrap();
    put_via_trait(&mut store, "env", env.clone()).unwrap();
    put_via_trait(&mut store, "key", key.clone()).unwrap();
    assert_eq!(store.get("op"), Some(op));
    assert_eq!(store.get("env"), Some(env));
    assert_eq!(store.get("key"), Some(key));
    assert_eq!(store.len(), 3);
}

#[test]
fn put_overwrites_same_name() {
    let mut store = MemoryAuthStore::new();
    store
        .put("grok", SecretRef::Keychain("old".into()))
        .unwrap();
    store
        .put("grok", SecretRef::Op("op://V/I/f".into()))
        .unwrap();
    assert_eq!(store.len(), 1);
    assert_eq!(store.get("grok"), Some(SecretRef::Op("op://V/I/f".into())));
}

#[test]
fn delete_removes_and_second_delete_is_not_found() {
    let mut store = MemoryAuthStore::new();
    store
        .put("grok", SecretRef::Keychain("acct".into()))
        .unwrap();
    store.delete("grok").unwrap();
    assert_eq!(store.get("grok"), None);
    assert!(store.is_empty());
    assert_eq!(
        store.delete("grok").unwrap_err(),
        AuthError::NotFound("grok".into())
    );
    assert_eq!(
        store.delete("grok").unwrap_err().to_string(),
        "not found: grok"
    );
}

#[test]
fn delete_missing_is_not_found() {
    let mut store = MemoryAuthStore::new();
    assert_eq!(
        store.delete("nope").unwrap_err(),
        AuthError::NotFound("nope".into())
    );
}

#[test]
fn put_rejects_constructed_plaintext_keychain() {
    let mut store = MemoryAuthStore::new();
    let raw = SecretRef::Keychain("sk-abcdefghijklmnopqrstu".into());
    assert!(raw.as_str().len() > 20);
    assert_eq!(
        store.put("leak", raw).unwrap_err(),
        AuthError::PlaintextForbidden
    );
    assert!(store.is_empty());
    assert_eq!(store.get("leak"), None);
}

#[test]
fn put_rejects_constructed_plaintext_op_and_env() {
    let mut store = MemoryAuthStore::new();
    assert_eq!(
        store
            .put("a", SecretRef::Op("sk-this-is-not-an-op-ref".into()))
            .unwrap_err(),
        AuthError::PlaintextForbidden
    );
    assert_eq!(
        store
            .put("b", SecretRef::Env("sk-this-is-not-an-env-ref".into()))
            .unwrap_err(),
        AuthError::PlaintextForbidden
    );
    assert_eq!(store.len(), 0);
}

#[test]
fn put_accepts_long_prefixed_refs() {
    let mut store = MemoryAuthStore::new();
    store
        .put(
            "op",
            SecretRef::Op("op://Vault/VeryLongItemName/field".into()),
        )
        .unwrap();
    store
        .put(
            "env",
            SecretRef::Env("${VERY_LONG_ENVIRONMENT_VARIABLE}".into()),
        )
        .unwrap();
    assert_eq!(store.len(), 2);
}

#[test]
fn put_accepts_twenty_char_keychain_rejects_twenty_one() {
    let mut store = MemoryAuthStore::new();
    let twenty = "abcdefghijklmnopqrst";
    let twenty_one = "abcdefghijklmnopqrstu";
    assert_eq!(twenty.len(), 20);
    assert_eq!(twenty_one.len(), 21);
    store.put("ok", SecretRef::Keychain(twenty.into())).unwrap();
    assert_eq!(
        store
            .put("bad", SecretRef::Keychain(twenty_one.into()))
            .unwrap_err(),
        AuthError::PlaintextForbidden
    );
    assert_eq!(store.get("ok"), Some(SecretRef::Keychain(twenty.into())));
    assert_eq!(store.get("bad"), None);
}

#[test]
fn parse_then_put_token_fails_before_store() {
    let store = MemoryAuthStore::new();
    let err = SecretRef::parse("0123456789012345678901").unwrap_err();
    assert_eq!(err, AuthError::PlaintextForbidden);
    assert_eq!(err.to_string(), "plaintext secrets are forbidden");
    assert!(store.is_empty());
}

proptest! {
    #[test]
    fn parsed_refs_round_trip(name in "[a-z]{1,8}", suffix in "[A-Za-z0-9_-]{1,12}") {
        let mut store = MemoryAuthStore::new();
        let op = format!("op://Vault/Item/{suffix}");
        let env = format!("${{{suffix}}}");
        let parsed_op = SecretRef::parse(&op).unwrap();
        let parsed_env = SecretRef::parse(&env).unwrap();
        store.put(&format!("{name}-op"), parsed_op.clone()).unwrap();
        store.put(&format!("{name}-env"), parsed_env.clone()).unwrap();
        prop_assert_eq!(store.get(&format!("{name}-op")), Some(parsed_op));
        prop_assert_eq!(store.get(&format!("{name}-env")), Some(parsed_env));
    }

    #[test]
    fn long_unprefixed_never_stored(raw in "[A-Za-z0-9]{21,40}") {
        prop_assume!(!raw.starts_with("op://"));
        prop_assume!(!raw.starts_with("${"));
        let mut store = MemoryAuthStore::new();
        prop_assert_eq!(
            SecretRef::parse(&raw).unwrap_err(),
            AuthError::PlaintextForbidden
        );
        prop_assert_eq!(
            store.put("x", SecretRef::Keychain(raw)).unwrap_err(),
            AuthError::PlaintextForbidden
        );
        prop_assert!(store.is_empty());
    }
}
