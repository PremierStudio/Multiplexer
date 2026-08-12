//! Multiplexer shared wire contract.
//!
//! Single source of truth for the JSON-RPC contract (D13/D20). The approval
//! model is implemented test-first (see `tests/approval_decision.rs`); the
//! JSON-RPC envelope, codec, error model, and event vocabulary are the Phase
//! 0.5 wire-contract skeleton (plan/04).

pub mod approval;
pub mod codec;
pub mod error;
pub mod event;
pub mod jsonrpc;
pub mod methods;
pub mod protocol;
