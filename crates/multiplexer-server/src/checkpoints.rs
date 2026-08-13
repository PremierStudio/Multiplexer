//! Optional checkpoint catalog used by `checkpoint.list` / `checkpoint.create` / `checkpoint.revert`.

use multiplexer_checkpoint::{Checkpoint, CheckpointError, CheckpointId, CheckpointStore};
use multiplexer_wire::error::AppErrorKind;
use serde::{Deserialize, Serialize};

use crate::backend::BackendError;

/// Wire-facing checkpoint row returned by `checkpoint.list` / `checkpoint.create` / `checkpoint.revert`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub label: String,
    pub seq: u64,
}

impl From<Checkpoint> for CheckpointInfo {
    fn from(checkpoint: Checkpoint) -> Self {
        Self {
            id: checkpoint.id.to_string(),
            label: checkpoint.label,
            seq: checkpoint.seq,
        }
    }
}

/// Session-scoped checkpoint list, create, and revert. Tests inject [`CheckpointStore`].
pub trait CheckpointCatalog: Send {
    fn list(&self, session_id: &str) -> Vec<CheckpointInfo>;
    fn create(&mut self, session_id: &str, label: &str) -> Result<CheckpointInfo, BackendError>;
    fn revert(&mut self, checkpoint_id: &str) -> Result<CheckpointInfo, BackendError>;
}

impl CheckpointCatalog for CheckpointStore {
    fn list(&self, session_id: &str) -> Vec<CheckpointInfo> {
        CheckpointStore::list(self, session_id)
            .into_iter()
            .map(CheckpointInfo::from)
            .collect()
    }

    fn create(&mut self, session_id: &str, label: &str) -> Result<CheckpointInfo, BackendError> {
        Ok(CheckpointInfo::from(CheckpointStore::create(
            self, session_id, label,
        )))
    }

    fn revert(&mut self, checkpoint_id: &str) -> Result<CheckpointInfo, BackendError> {
        CheckpointStore::revert(self, &CheckpointId::from(checkpoint_id))
            .map(CheckpointInfo::from)
            .map_err(|err| match err {
                CheckpointError::NotFound(id) => BackendError::Provider {
                    kind: AppErrorKind::NotFound,
                    message: format!("checkpoint not found: {id}"),
                },
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_copies_id_label_seq() {
        let info = CheckpointInfo::from(Checkpoint {
            id: CheckpointId::from("cp-3"),
            session_id: "sess".into(),
            label: "turn".into(),
            seq: 3,
        });
        assert_eq!(info.id, "cp-3");
        assert_eq!(info.label, "turn");
        assert_eq!(info.seq, 3);
    }

    #[test]
    fn list_is_session_scoped_and_ordered() {
        let mut store = CheckpointStore::new();
        store.create("alpha", "start");
        store.create("beta", "other");
        store.create("alpha", "turn");
        let alpha = CheckpointCatalog::list(&store, "alpha");
        let beta = CheckpointCatalog::list(&store, "beta");
        assert_eq!(
            alpha,
            vec![
                CheckpointInfo {
                    id: "cp-1".into(),
                    label: "start".into(),
                    seq: 1,
                },
                CheckpointInfo {
                    id: "cp-3".into(),
                    label: "turn".into(),
                    seq: 2,
                },
            ]
        );
        assert_eq!(
            beta,
            vec![CheckpointInfo {
                id: "cp-2".into(),
                label: "other".into(),
                seq: 1,
            }]
        );
        assert!(CheckpointCatalog::list(&store, "missing").is_empty());
    }

    #[test]
    fn create_appends_and_sets_current() {
        let mut store = CheckpointStore::new();
        let first = CheckpointCatalog::create(&mut store, "s", "a").expect("create");
        assert_eq!(
            first,
            CheckpointInfo {
                id: "cp-1".into(),
                label: "a".into(),
                seq: 1,
            }
        );
        assert_eq!(
            store.current("s").as_ref().map(|id| id.as_str()),
            Some("cp-1")
        );

        let second = CheckpointCatalog::create(&mut store, "s", "b").expect("create");
        assert_eq!(
            second,
            CheckpointInfo {
                id: "cp-2".into(),
                label: "b".into(),
                seq: 2,
            }
        );
        assert_eq!(
            store.current("s").as_ref().map(|id| id.as_str()),
            Some("cp-2")
        );
        assert_eq!(
            CheckpointCatalog::list(&store, "s"),
            vec![first.clone(), second]
        );
    }

    #[test]
    fn revert_maps_row() {
        let mut store = CheckpointStore::new();
        store.create("s", "a");
        store.create("s", "b");
        let row = CheckpointCatalog::revert(&mut store, "cp-1").expect("revert");
        assert_eq!(
            row,
            CheckpointInfo {
                id: "cp-1".into(),
                label: "a".into(),
                seq: 1,
            }
        );
        assert_eq!(
            store.current("s").as_ref().map(|id| id.as_str()),
            Some("cp-1")
        );
        assert_eq!(CheckpointCatalog::list(&store, "s").len(), 2);
    }

    #[test]
    fn revert_unknown_is_not_found() {
        let mut store = CheckpointStore::new();
        store.create("s", "keep");
        let err = CheckpointCatalog::revert(&mut store, "cp-99").unwrap_err();
        assert!(matches!(
            err,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::NotFound && message.contains("cp-99")
        ));
        assert_eq!(
            store.current("s").as_ref().map(|id| id.as_str()),
            Some("cp-1")
        );
    }
}
