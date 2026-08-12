//! Protocol versioning (plan/04 §9.2).
//!
//! `protocol_version` is a semver string. Major = breaking change (server and
//! client must match); minor = additive (negotiated). The server is the source
//! of truth; on mismatch it rejects with `protocol_version_mismatch`.

/// The current wire protocol version, derived from the schema crate.
pub const PROTOCOL_VERSION: &str = "0.1.0";
