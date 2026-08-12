//! In-memory checkpoint table. No git refs are written.

use std::collections::HashMap;

use crate::{CheckpointError, CheckpointId};

/// One captured workspace pointer (hidden-ref stand-in).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub id: CheckpointId,
    pub session_id: String,
    pub label: String,
    /// 1-based creation order within [`Self::session_id`].
    pub seq: u64,
}

/// Session-scoped list of checkpoints plus a current pointer per session.
#[derive(Debug)]
pub struct CheckpointStore {
    next: u64,
    checkpoints: Vec<Checkpoint>,
    current: HashMap<String, CheckpointId>,
}

impl Default for CheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointStore {
    /// Empty store. The first created id is `cp-1`.
    pub fn new() -> Self {
        Self {
            next: 1,
            checkpoints: Vec::new(),
            current: HashMap::new(),
        }
    }

    /// Append a checkpoint. Ids increment globally (`cp-1`, `cp-2`, ...).
    ///
    /// The new id becomes [`Self::current`] for `session_id`.
    pub fn create(&mut self, session_id: &str, label: &str) -> Checkpoint {
        let seq = self
            .checkpoints
            .iter()
            .filter(|cp| cp.session_id == session_id)
            .count() as u64
            + 1;
        let id = CheckpointId(format!("cp-{}", self.next));
        self.next += 1;
        let checkpoint = Checkpoint {
            id: id.clone(),
            session_id: session_id.to_owned(),
            label: label.to_owned(),
            seq,
        };
        self.checkpoints.push(checkpoint.clone());
        self.current.insert(session_id.to_owned(), id);
        checkpoint
    }

    /// Checkpoints for `session_id` in creation order.
    pub fn list(&self, session_id: &str) -> Vec<Checkpoint> {
        self.checkpoints
            .iter()
            .filter(|cp| cp.session_id == session_id)
            .cloned()
            .collect()
    }

    /// Lookup by global id.
    pub fn get(&self, id: &CheckpointId) -> Option<Checkpoint> {
        self.checkpoints.iter().find(|cp| &cp.id == id).cloned()
    }

    /// Point the owning session's current pointer at `id`.
    ///
    /// Unknown ids yield [`CheckpointError::NotFound`]. Existing checkpoints
    /// are kept (revert is not a truncate).
    pub fn revert(&mut self, id: &CheckpointId) -> Result<Checkpoint, CheckpointError> {
        let checkpoint = self
            .get(id)
            .ok_or_else(|| CheckpointError::NotFound(id.clone()))?;
        self.current
            .insert(checkpoint.session_id.clone(), checkpoint.id.clone());
        Ok(checkpoint)
    }

    /// Current pointer for `session_id`, if any checkpoint exists (or was reverted to).
    pub fn current(&self, session_id: &str) -> Option<CheckpointId> {
        self.current.get(session_id).cloned()
    }
}
