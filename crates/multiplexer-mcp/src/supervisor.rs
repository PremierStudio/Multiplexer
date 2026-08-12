//! Reference-counted MCP instance table and crash/backoff transitions.

use std::collections::{HashMap, HashSet};

use crate::config::{config_hash, ConfigHash, ServerConfig, ServerId};

/// Consecutive crashes that mark an instance permanently [`LifecycleState::Failed`].
pub const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Base backoff in milliseconds (1s).
pub const BACKOFF_BASE_MS: u32 = 1_000;

/// Cap on exponential backoff (30s), per plan/21 D44.
pub const BACKOFF_CAP_MS: u32 = 30_000;

/// `min(30_000, 1_000 * 2^restarts)`.
///
/// `restarts` is the 0-based crash index: 0 => 1000, 1 => 2000, 2 => 4000,
/// 3 => 8000, 4 => 16000, 5+ => 30000. The supervisor fails at 5 consecutive
/// crashes, so the live restart backoffs are 1000, 2000, 4000, 8000.
pub fn backoff_ms_for(restarts: u32) -> u32 {
    if restarts >= 5 {
        BACKOFF_CAP_MS
    } else {
        BACKOFF_BASE_MS << restarts
    }
}

/// Lifecycle of one supervised instance (plan/21 §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Spawned,
    Ready,
    Crashed { restarts: u32 },
    Stopped,
    Failed,
}

/// Held reference to a live (or failed-but-not-yet-released) instance.
///
/// Not [`Clone`]: each acquire produces one token that [`Supervisor::release`]
/// consumes. Dropping a handle without release leaks the reference.
#[derive(Debug, PartialEq, Eq)]
pub struct ServerHandle {
    id: ServerId,
    hash: ConfigHash,
    token: u64,
}

impl ServerHandle {
    pub fn id(&self) -> &ServerId {
        &self.id
    }

    pub fn hash(&self) -> ConfigHash {
        self.hash
    }
}

/// Errors from supervisor operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SupervisorError {
    #[error("unknown server '{0}'")]
    UnknownServer(String),
    #[error("unknown handle for server '{0}'")]
    UnknownHandle(String),
    #[error("illegal transition from {from:?}")]
    IllegalTransition { from: LifecycleState },
}

struct Instance {
    id: ServerId,
    hash: ConfigHash,
    state: LifecycleState,
    restarts: u32,
    last_backoff_ms: Option<u32>,
    tokens: HashSet<u64>,
    seq: u64,
}

/// In-memory supervisor. Spawn is instant (Spawned then Ready).
pub struct Supervisor {
    instances: HashMap<ConfigHash, Instance>,
    /// Insertion-ordered hashes per configured name so name lookup is deterministic.
    names: HashMap<ServerId, Vec<ConfigHash>>,
    next_token: u64,
    next_seq: u64,
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Supervisor {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            names: HashMap::new(),
            next_token: 1,
            next_seq: 1,
        }
    }

    /// Attach to a live instance with this config hash, or spawn a new one.
    ///
    /// A [`LifecycleState::Failed`] instance is not reused. If it is still
    /// referenced, acquire starts a new incarnation on the same slot (handles
    /// already held stay valid). After the last release the slot is gone, so
    /// the next acquire is a fresh instance.
    pub fn acquire(&mut self, cfg: &ServerConfig) -> ServerHandle {
        let hash = config_hash(cfg);
        let id = cfg.server_id();
        if let Some(inst) = self.instances.get_mut(&hash) {
            if matches!(inst.state, LifecycleState::Failed) {
                inst.restarts = 0;
                inst.last_backoff_ms = None;
                inst.state = LifecycleState::Ready;
            }
            return self.attach(hash);
        }
        self.spawn_new(id, hash)
    }

    /// Drop one reference. At zero the instance is removed (Stopped, not kept).
    pub fn release(&mut self, handle: ServerHandle) -> Result<(), SupervisorError> {
        let ServerHandle { id, hash, token } = handle;
        let inst = self
            .instances
            .get_mut(&hash)
            .ok_or_else(|| SupervisorError::UnknownHandle(id.to_string()))?;
        if !inst.tokens.remove(&token) {
            return Err(SupervisorError::UnknownHandle(id.to_string()));
        }
        if !inst.tokens.is_empty() {
            return Ok(());
        }
        self.instances.remove(&hash);
        if let Some(hashes) = self.names.get_mut(&id) {
            hashes.retain(|h| *h != hash);
            if hashes.is_empty() {
                self.names.remove(&id);
            }
        }
        Ok(())
    }

    /// Record an unexpected exit. Restarts with backoff while under the
    /// consecutive-failure cap; the fifth crash is [`LifecycleState::Failed`].
    pub fn mark_crashed(&mut self, id: &ServerId) -> Result<LifecycleState, SupervisorError> {
        let hash = self
            .lookup_hash(id)
            .ok_or_else(|| SupervisorError::UnknownServer(id.to_string()))?;
        self.mark_crashed_hash(&hash)
    }

    /// Crash the instance identified by reuse key.
    pub fn mark_crashed_hash(
        &mut self,
        hash: &ConfigHash,
    ) -> Result<LifecycleState, SupervisorError> {
        let inst = self
            .instances
            .get_mut(hash)
            .ok_or_else(|| SupervisorError::UnknownServer(hash.to_hex()))?;
        match inst.state {
            LifecycleState::Failed | LifecycleState::Stopped => {
                return Err(SupervisorError::IllegalTransition { from: inst.state });
            }
            LifecycleState::Spawned | LifecycleState::Ready | LifecycleState::Crashed { .. } => {}
        }
        inst.restarts = inst.restarts.saturating_add(1);
        if inst.restarts >= MAX_CONSECUTIVE_FAILURES {
            inst.last_backoff_ms = None;
            inst.state = LifecycleState::Failed;
            return Ok(inst.state);
        }
        let backoff = backoff_ms_for(inst.restarts - 1);
        inst.last_backoff_ms = Some(backoff);
        // Instant fake respawn: Crashed then Spawned then Ready.
        inst.state = LifecycleState::Ready;
        Ok(inst.state)
    }

    pub fn refcount(&self, id: &ServerId) -> Option<u32> {
        self.lookup(id).map(|inst| inst.tokens.len() as u32)
    }

    pub fn refcount_hash(&self, hash: &ConfigHash) -> Option<u32> {
        self.instances
            .get(hash)
            .map(|inst| inst.tokens.len() as u32)
    }

    pub fn state(&self, id: &ServerId) -> Option<LifecycleState> {
        self.lookup(id).map(|inst| inst.state)
    }

    pub fn state_hash(&self, hash: &ConfigHash) -> Option<LifecycleState> {
        self.instances.get(hash).map(|inst| inst.state)
    }

    pub fn restart_count(&self, id: &ServerId) -> Option<u32> {
        self.lookup(id).map(|inst| inst.restarts)
    }

    pub fn backoff_ms(&self, id: &ServerId) -> Option<u32> {
        self.lookup(id).and_then(|inst| inst.last_backoff_ms)
    }

    /// Number of instances still in the table (including Failed until released).
    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    fn attach(&mut self, hash: ConfigHash) -> ServerHandle {
        let token = self.alloc_token();
        let inst = self
            .instances
            .get_mut(&hash)
            .expect("attach requires a live instance");
        inst.tokens.insert(token);
        ServerHandle {
            id: inst.id.clone(),
            hash,
            token,
        }
    }

    fn spawn_new(&mut self, id: ServerId, hash: ConfigHash) -> ServerHandle {
        let token = self.alloc_token();
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        let mut tokens = HashSet::new();
        tokens.insert(token);
        // Instant fake spawn: Spawned is not observable after acquire returns.
        let inst = Instance {
            id: id.clone(),
            hash,
            state: LifecycleState::Ready,
            restarts: 0,
            last_backoff_ms: None,
            tokens,
            seq,
        };
        self.instances.insert(hash, inst);
        self.names.entry(id.clone()).or_default().push(hash);
        ServerHandle { id, hash, token }
    }

    fn alloc_token(&mut self) -> u64 {
        let token = self.next_token;
        self.next_token = self.next_token.saturating_add(1);
        token
    }

    fn lookup(&self, id: &ServerId) -> Option<&Instance> {
        let hashes = self.names.get(id)?;
        let mut best: Option<&Instance> = None;
        for hash in hashes {
            let Some(inst) = self.instances.get(hash) else {
                continue;
            };
            best = Some(match best {
                None => inst,
                Some(prev) => preferred(prev, inst),
            });
        }
        best
    }

    fn lookup_hash(&self, id: &ServerId) -> Option<ConfigHash> {
        self.lookup(id).map(|inst| inst.hash)
    }
}

fn preferred<'a>(a: &'a Instance, b: &'a Instance) -> &'a Instance {
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

fn rank(inst: &Instance) -> (u8, u64) {
    let reusable = match inst.state {
        LifecycleState::Failed | LifecycleState::Stopped => 0,
        LifecycleState::Spawned | LifecycleState::Ready | LifecycleState::Crashed { .. } => 1,
    };
    (reusable, inst.seq)
}
