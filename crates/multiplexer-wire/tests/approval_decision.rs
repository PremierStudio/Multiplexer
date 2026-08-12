//! Tests for the approval-decision model (D12).
//!
//! TDD: written FIRST (red), implementation added to make them pass (green).
//! The 4-way decision enum is the single most cross-cutting type in the
//! product (wire, adapter, orchestration, security), so it gets deep,
//! property-based tests: serialization round-trips, parse/reject behavior,
//! display strings, and the allow/deny/remember semantics.

use multiplexer_wire::approval::{ApprovalDecision, ApprovalDecisionParseError};
use proptest::prop_assert_eq;

// ---------------------------------------------------------------------------
// 1. Constructors and identity
// ---------------------------------------------------------------------------

#[test]
fn all_four_variants_construct() {
    let allow = ApprovalDecision::Allow;
    let deny = ApprovalDecision::Deny;
    let once = ApprovalDecision::AllowOnce;
    let always = ApprovalDecision::AllowAlways;

    assert!(matches!(allow, ApprovalDecision::Allow));
    assert!(matches!(deny, ApprovalDecision::Deny));
    assert!(matches!(once, ApprovalDecision::AllowOnce));
    assert!(matches!(always, ApprovalDecision::AllowAlways));
}

// ---------------------------------------------------------------------------
// 2. Semantics: does this decision permit the action? Does it remember?
// ---------------------------------------------------------------------------

#[test]
fn permits_reflect_the_four_way_semantics() {
    assert!(ApprovalDecision::Allow.permits());
    assert!(ApprovalDecision::AllowOnce.permits());
    assert!(ApprovalDecision::AllowAlways.permits());
    assert!(!ApprovalDecision::Deny.permits());
}

#[test]
fn remembers_only_allow_always() {
    assert!(!ApprovalDecision::Allow.remembers());
    assert!(!ApprovalDecision::Deny.remembers());
    assert!(!ApprovalDecision::AllowOnce.remembers());
    assert!(ApprovalDecision::AllowAlways.remembers());
}

// ---------------------------------------------------------------------------
// 3. Wire serialization (D12: the 4-way enum is carried verbatim on the wire)
// ---------------------------------------------------------------------------

#[test]
fn serializes_to_canonical_wire_names() {
    assert_eq!(
        serde_json::to_string(&ApprovalDecision::Allow).unwrap(),
        "\"allow\""
    );
    assert_eq!(
        serde_json::to_string(&ApprovalDecision::Deny).unwrap(),
        "\"deny\""
    );
    assert_eq!(
        serde_json::to_string(&ApprovalDecision::AllowOnce).unwrap(),
        "\"allow_once\""
    );
    assert_eq!(
        serde_json::to_string(&ApprovalDecision::AllowAlways).unwrap(),
        "\"allow_always\""
    );
}

#[test]
fn deserializes_from_canonical_wire_names() {
    assert_eq!(
        serde_json::from_str::<ApprovalDecision>("\"allow\"").unwrap(),
        ApprovalDecision::Allow
    );
    assert_eq!(
        serde_json::from_str::<ApprovalDecision>("\"deny\"").unwrap(),
        ApprovalDecision::Deny
    );
    assert_eq!(
        serde_json::from_str::<ApprovalDecision>("\"allow_once\"").unwrap(),
        ApprovalDecision::AllowOnce
    );
    assert_eq!(
        serde_json::from_str::<ApprovalDecision>("\"allow_always\"").unwrap(),
        ApprovalDecision::AllowAlways
    );
}

#[test]
fn round_trips_all_variants() {
    for d in [
        ApprovalDecision::Allow,
        ApprovalDecision::Deny,
        ApprovalDecision::AllowOnce,
        ApprovalDecision::AllowAlways,
    ] {
        let json = serde_json::to_string(&d).unwrap();
        let back: ApprovalDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d, "round-trip failed for {d:?}");
    }
}

// ---------------------------------------------------------------------------
// 4. Property-based: serde round-trip is identity for every variant
// ---------------------------------------------------------------------------

proptest::proptest! {
    #[test]
    fn serde_round_trip_is_identity(d in proptest::prop_oneof![
        proptest::strategy::Just(ApprovalDecision::Allow),
        proptest::strategy::Just(ApprovalDecision::Deny),
        proptest::strategy::Just(ApprovalDecision::AllowOnce),
        proptest::strategy::Just(ApprovalDecision::AllowAlways),
    ]) {
        let json = serde_json::to_string(&d).expect("serialize");
        let back: ApprovalDecision = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(back, d);
        prop_assert_eq!(back.permits(), d.permits());
        prop_assert_eq!(back.remembers(), d.remembers());
    }
}

// ---------------------------------------------------------------------------
// 5. Parse from wire string (what the JSON-RPC layer sees)
// ---------------------------------------------------------------------------

#[test]
fn parses_all_canonical_spellings() {
    for (s, expected) in [
        ("allow", ApprovalDecision::Allow),
        ("deny", ApprovalDecision::Deny),
        ("allow_once", ApprovalDecision::AllowOnce),
        ("allow_always", ApprovalDecision::AllowAlways),
    ] {
        assert_eq!(ApprovalDecision::parse(s), Ok(expected), "parse({s:?})");
    }
}

#[test]
fn from_str_impl_delegates_to_parse() {
    use std::str::FromStr;
    // The FromStr trait is the generic-parse seam (used by clap-style and
    // config parsing); it must behave identically to ApprovalDecision::parse.
    for (s, expected) in [
        ("allow", ApprovalDecision::Allow),
        ("deny", ApprovalDecision::Deny),
        ("allow_once", ApprovalDecision::AllowOnce),
        ("allow_always", ApprovalDecision::AllowAlways),
    ] {
        let parsed = ApprovalDecision::from_str(s);
        assert_eq!(parsed, Ok(expected), "from_str({s:?})");
        assert_eq!(parsed, s.parse::<ApprovalDecision>(), "str::parse({s:?})");
    }
    let err = ApprovalDecision::from_str("nope").unwrap_err();
    assert!(matches!(err, ApprovalDecisionParseError::UnknownVariant(_)));
    assert!(matches!(
        "nope".parse::<ApprovalDecision>(),
        Err(ApprovalDecisionParseError::UnknownVariant(_))
    ));
}

#[test]
fn rejects_garbage_and_unknown_spellings() {
    for bad in [
        "",
        " ",
        "Allow",
        "ALLOW",
        "allow-once",
        "approve",
        "maybe",
        "1",
        "true",
        "allow\nonce",
    ] {
        assert!(
            matches!(
                ApprovalDecision::parse(bad),
                Err(ApprovalDecisionParseError::UnknownVariant(_))
            ),
            "parse({bad:?}) should reject with UnknownVariant"
        );
    }
}

#[test]
fn rejects_non_string_json() {
    // None of these are a valid string enum value, so deserialization must
    // fail (the exact error category is serde's, not ours — the invariant is
    // that they are rejected, never accepted).
    for bad in ["null", "1", "true", "[1,2]", "{}"] {
        let err = serde_json::from_str::<ApprovalDecision>(bad).unwrap_err();
        let _ = err.to_string(); // error is Display-able
        assert!(
            serde_json::from_str::<ApprovalDecision>(bad).is_err(),
            "json {bad:?} must be rejected"
        );
    }
}

#[test]
fn parse_error_display_is_actionable() {
    let e = ApprovalDecision::parse("approve").unwrap_err();
    let msg = e.to_string();
    assert!(
        msg.contains("approve"),
        "message should name the bad input: {msg}"
    );
    assert!(
        msg.contains("allow")
            && msg.contains("deny")
            && msg.contains("allow_once")
            && msg.contains("allow_always"),
        "message should list all valid variants: {msg}"
    );
}

// ---------------------------------------------------------------------------
// 6. Display for logs / UI
// ---------------------------------------------------------------------------

#[test]
fn display_matches_canonical_names() {
    assert_eq!(ApprovalDecision::Allow.to_string(), "allow");
    assert_eq!(ApprovalDecision::Deny.to_string(), "deny");
    assert_eq!(ApprovalDecision::AllowOnce.to_string(), "allow_once");
    assert_eq!(ApprovalDecision::AllowAlways.to_string(), "allow_always");
}

// ---------------------------------------------------------------------------
// 7. Structural invariants
// ---------------------------------------------------------------------------

#[test]
fn variants_are_exhaustive_and_distinct() {
    let all = [
        ApprovalDecision::Allow,
        ApprovalDecision::Deny,
        ApprovalDecision::AllowOnce,
        ApprovalDecision::AllowAlways,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "{a:?} and {b:?} must be distinct");
            }
        }
    }
    assert_eq!(all.len(), 4, "exactly four variants, never more");
}
