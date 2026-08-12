//! Multiplexer core resource manager: CPU core bitmap allocation for sessions.

#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

pub mod bitmap;
pub mod containment;
pub mod manager;
pub mod telemetry;

pub use bitmap::{CoreBitmap, ResmanError, SessionAlloc, SessionId};
pub use containment::{
    ChildId, ContainedChild, Containment, ContainmentError, FakeContainment, FakeWatch, SpawnSpec,
};
pub use manager::{ManagerError, ResourceManager};
pub use telemetry::{format_core_bar, sample_cores, sample_cores_from, CoreSample};

#[cfg(windows)]
pub use containment::{
    job_err, pid_is_alive, query_still_active, working_set_limit, JobContainment, PAGE,
};
