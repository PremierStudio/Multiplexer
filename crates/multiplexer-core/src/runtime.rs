//! [`SessionRuntime`]: start/stop wires provider, resman, and checkpoints.

use std::collections::HashMap;
use std::path::PathBuf;

use multiplexer_checkpoint::CheckpointStore;
use multiplexer_provider::{
    FakeProvider, ProviderAdapter, ProviderError, SessionId, SessionStartParams,
};
use multiplexer_resman::{
    FakeContainment, ManagerError, ResourceManager, SessionId as ResmanSessionId, SpawnSpec,
};

/// Resource manager used by the fake runtime.
pub type FakeResourceManager = ResourceManager<FakeContainment, fn() -> FakeContainment>;

/// Cores granted to each started session (bitmap has 8 total, 0-1 reserved).
const CORES_PER_SESSION: usize = 1;

/// Failures from [`SessionRuntime::start`] / [`SessionRuntime::stop`].
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionRuntimeError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    Resman(#[from] ManagerError),
}

/// Owns a [`FakeProvider`], a fake [`ResourceManager`], and a [`CheckpointStore`].
pub struct SessionRuntime {
    provider: FakeProvider,
    resman: FakeResourceManager,
    checkpoints: CheckpointStore,
    resman_ids: HashMap<String, ResmanSessionId>,
    next_resman: u64,
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRuntime {
    /// Empty runtime: 8-core fake resman, empty provider and checkpoint store.
    pub fn new() -> Self {
        Self {
            provider: FakeProvider::new(),
            resman: ResourceManager::fake(8).expect("8 cores is a valid bitmap"),
            checkpoints: CheckpointStore::new(),
            resman_ids: HashMap::new(),
            next_resman: 1,
        }
    }

    pub fn provider(&self) -> &FakeProvider {
        &self.provider
    }

    pub fn resman(&self) -> &FakeResourceManager {
        &self.resman
    }

    pub fn checkpoints(&self) -> &CheckpointStore {
        &self.checkpoints
    }

    /// Start a provider session, allocate fake containment, create a "start" checkpoint.
    pub fn start(&mut self, params: SessionStartParams) -> Result<SessionId, SessionRuntimeError> {
        let id = self.provider.start_session(params)?;
        let rid = ResmanSessionId(self.next_resman);
        self.next_resman += 1;
        if let Err(err) = self
            .resman
            .start_session(rid, CORES_PER_SESSION, dummy_spec())
        {
            let _ = self.provider.session_stop(&id);
            return Err(err.into());
        }
        self.resman_ids.insert(id.as_str().to_owned(), rid);
        self.checkpoints.create(id.as_str(), "start");
        Ok(id)
    }

    /// Stop the provider session and free its resource allocation.
    ///
    /// Checkpoints are kept (history is not truncated).
    pub fn stop(&mut self, session: &SessionId) -> Result<(), SessionRuntimeError> {
        self.provider.session_stop(session)?;
        let rid = self
            .resman_ids
            .remove(session.as_str())
            .ok_or_else(|| ProviderError::NotFound(format!("session {session}")))?;
        self.resman.stop_session(rid)?;
        Ok(())
    }
}

fn dummy_spec() -> SpawnSpec {
    SpawnSpec {
        program: PathBuf::from("dummy"),
        args: Vec::new(),
        memory_cap_bytes: None,
    }
}
