//! Session-level resource manager: core bitmap plus per-session containment.

use std::collections::HashMap;

use crate::bitmap::{CoreBitmap, ResmanError, SessionAlloc, SessionId};
use crate::containment::{
    ChildId, ContainedChild, Containment, ContainmentError, FakeContainment, SpawnSpec,
};

/// Errors from [`ResourceManager`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManagerError {
    #[error(transparent)]
    Bitmap(#[from] ResmanError),
    #[error(transparent)]
    Containment(#[from] ContainmentError),
    #[error("unknown session {0}")]
    UnknownSession(u64),
}

struct Slot<C> {
    containment: C,
    child: ContainedChild,
}

/// Binds CPU allocation to a kill-on-close containment per session.
pub struct ResourceManager<C: Containment, F> {
    bitmap: CoreBitmap,
    factory: F,
    sessions: HashMap<u64, Slot<C>>,
}

impl ResourceManager<FakeContainment, fn() -> FakeContainment> {
    /// Manager that uses in-memory containment (tests and default local stub).
    pub fn fake(n_cores: usize) -> Result<Self, ManagerError> {
        let bitmap = CoreBitmap::new(n_cores)?;
        Ok(Self {
            bitmap,
            factory: FakeContainment::new,
            sessions: HashMap::new(),
        })
    }
}

impl<C, F> ResourceManager<C, F>
where
    C: Containment,
    F: Fn() -> C,
{
    pub fn new(n_cores: usize, factory: F) -> Result<Self, ManagerError> {
        let bitmap = CoreBitmap::new(n_cores)?;
        Ok(Self {
            bitmap,
            factory,
            sessions: HashMap::new(),
        })
    }

    pub fn start_session(
        &mut self,
        id: SessionId,
        cores: usize,
        spec: SpawnSpec,
    ) -> Result<SessionAlloc, ManagerError> {
        let alloc = self.bitmap.allocate(id, cores, spec.memory_cap_bytes)?;
        let mut containment = (self.factory)();
        match containment.spawn(spec) {
            Ok(child) => {
                self.sessions.insert(id.0, Slot { containment, child });
                Ok(alloc)
            }
            Err(e) => {
                let _ = self.bitmap.free(id);
                Err(e.into())
            }
        }
    }

    pub fn stop_session(&mut self, id: SessionId) -> Result<SessionAlloc, ManagerError> {
        let slot = self
            .sessions
            .remove(&id.0)
            .ok_or(ManagerError::UnknownSession(id.0))?;
        drop(slot.containment);
        Ok(self.bitmap.free(id)?)
    }

    pub fn session_alive(&self, id: SessionId) -> Result<bool, ManagerError> {
        let slot = self
            .sessions
            .get(&id.0)
            .ok_or(ManagerError::UnknownSession(id.0))?;
        Ok(slot.containment.child_alive(slot.child.id)?)
    }

    pub fn child_id(&self, id: SessionId) -> Result<ChildId, ManagerError> {
        self.sessions
            .get(&id.0)
            .map(|s| s.child.id)
            .ok_or(ManagerError::UnknownSession(id.0))
    }

    pub fn alloc_of(&self, id: SessionId) -> Option<&SessionAlloc> {
        self.bitmap.allocated(id)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn free_enabled_count(&self) -> usize {
        self.bitmap.free_enabled_count()
    }
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod manager_tests;
