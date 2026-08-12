//! Session composition: provider + resource manager + checkpoint store.
//!
//! Orchestration (decider/projector) lands later. This crate owns the
//! testable start/stop runtime used by the server wrapper.

mod runtime;

pub use runtime::{FakeResourceManager, SessionRuntime, SessionRuntimeError};
