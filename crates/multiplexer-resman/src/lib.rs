//! Multiplexer core resource manager: CPU core bitmap allocation for sessions.

pub mod bitmap;
pub mod containment;
pub mod manager;

pub use bitmap::{CoreBitmap, ResmanError, SessionAlloc, SessionId};
pub use containment::{
    ChildId, ContainedChild, Containment, ContainmentError, FakeContainment, FakeWatch, SpawnSpec,
};
pub use manager::{ManagerError, ResourceManager};

#[cfg(windows)]
pub use containment::JobContainment;
