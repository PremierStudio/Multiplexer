use std::collections::HashMap;

/// Unique identifier for a session that owns a set of cores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Result of allocating cores to a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionAlloc {
    pub session: SessionId,
    pub cores: Vec<usize>,
    pub memory_cap_bytes: Option<u64>,
}

/// Errors returned by [`CoreBitmap`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResmanError {
    #[error("core count must be at least 1")]
    InvalidCoreCount,
    #[error("core {0} out of range")]
    CoreOutOfRange(usize),
    #[error("core {0} is reserved")]
    CoreReserved(usize),
    #[error("not enough free cores: need {needed}, have {available}")]
    InsufficientCores { needed: usize, available: usize },
    #[error("session {0} already allocated")]
    SessionAlreadyAllocated(u64),
    #[error("unknown session {0}")]
    UnknownSession(u64),
}

/// Tracks enabled, reserved, and session-allocated cores.
///
/// Cores 0 and 1 are reserved when the bitmap has more than 2 cores, otherwise
/// nothing is reserved. A core is "free" when it is enabled, not reserved, and
/// not owned by any session allocation.
pub struct CoreBitmap {
    n_cores: usize,
    enabled: Vec<bool>,
    manually_reserved: Vec<bool>,
    allocations: HashMap<u64, SessionAlloc>,
}

impl CoreBitmap {
    /// `n_cores` total. Cores 0 and 1 reserved if `n_cores > 2`, else none reserved.
    pub fn new(n_cores: usize) -> Result<Self, ResmanError> {
        if n_cores == 0 {
            return Err(ResmanError::InvalidCoreCount);
        }
        Ok(Self {
            n_cores,
            enabled: vec![true; n_cores],
            manually_reserved: vec![false; n_cores],
            allocations: HashMap::new(),
        })
    }

    fn statically_reserved(&self, core: usize) -> bool {
        self.n_cores > 2 && core < 2
    }

    fn is_reserved(&self, core: usize) -> bool {
        self.statically_reserved(core) || self.manually_reserved[core]
    }

    /// Mark cores as reserved. The call is atomic: it fails with the first bad
    /// core and applies nothing, and repeating a core within one call is fine.
    pub fn reserve(&mut self, cores: &[usize]) -> Result<(), ResmanError> {
        for &core in cores {
            if core >= self.n_cores {
                return Err(ResmanError::CoreOutOfRange(core));
            }
            if self.is_reserved(core) {
                return Err(ResmanError::CoreReserved(core));
            }
        }
        for &core in cores {
            self.manually_reserved[core] = true;
        }
        Ok(())
    }

    pub fn set_enabled(&mut self, core: usize, enabled: bool) -> Result<(), ResmanError> {
        if core >= self.n_cores {
            return Err(ResmanError::CoreOutOfRange(core));
        }
        self.enabled[core] = enabled;
        Ok(())
    }

    /// Allocate `count` free, enabled, non-reserved cores to `session`,
    /// preferring lowest indices.
    pub fn allocate(
        &mut self,
        session: SessionId,
        count: usize,
        memory_cap_bytes: Option<u64>,
    ) -> Result<SessionAlloc, ResmanError> {
        if self.allocations.contains_key(&session.0) {
            return Err(ResmanError::SessionAlreadyAllocated(session.0));
        }
        let free = self.free_cores();
        if free.len() < count {
            return Err(ResmanError::InsufficientCores {
                needed: count,
                available: free.len(),
            });
        }
        let alloc = SessionAlloc {
            session,
            cores: free.into_iter().take(count).collect(),
            memory_cap_bytes,
        };
        self.allocations.insert(alloc.session.0, alloc.clone());
        Ok(alloc)
    }

    pub fn free(&mut self, session: SessionId) -> Result<SessionAlloc, ResmanError> {
        self.allocations
            .remove(&session.0)
            .ok_or(ResmanError::UnknownSession(session.0))
    }

    pub fn allocated(&self, session: SessionId) -> Option<&SessionAlloc> {
        self.allocations.get(&session.0)
    }

    pub fn free_enabled_count(&self) -> usize {
        self.free_cores().len()
    }

    pub fn enabled_non_reserved_count(&self) -> usize {
        (0..self.n_cores)
            .filter(|&c| self.enabled[c] && !self.is_reserved(c))
            .count()
    }

    fn free_cores(&self) -> Vec<usize> {
        (0..self.n_cores)
            .filter(|&c| {
                self.enabled[c]
                    && !self.is_reserved(c)
                    && !self.allocations.values().any(|a| a.cores.contains(&c))
            })
            .collect()
    }
}
