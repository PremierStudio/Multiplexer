//! Optional git catalog used by `git.worktrees`.

use std::path::Path;

use multiplexer_wire::error::AppErrorKind;
use multiplexer_worktree::{GitRunner, Worktree, WorktreeService};
use serde::{Deserialize, Serialize};

use crate::backend::BackendError;

/// Wire-facing worktree row returned by `git.worktrees`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
}

impl From<Worktree> for WorktreeInfo {
    fn from(tree: Worktree) -> Self {
        Self {
            path: tree.path,
            head: tree.head,
            branch: tree.branch,
            detached: tree.detached,
            locked: tree.locked,
            prunable: tree.prunable,
        }
    }
}

/// Lists worktrees for a working directory. Tests inject [`WorktreeService<FakeGit>`].
pub trait GitCatalog: Send {
    fn list_worktrees(&self, cwd: &str) -> Result<Vec<WorktreeInfo>, BackendError>;
}

impl<R: GitRunner + Send> GitCatalog for WorktreeService<R> {
    fn list_worktrees(&self, cwd: &str) -> Result<Vec<WorktreeInfo>, BackendError> {
        self.list(Path::new(cwd))
            .map(|trees| trees.into_iter().map(WorktreeInfo::from).collect())
            .map_err(|err| BackendError::Provider {
                kind: AppErrorKind::ProviderError,
                message: err.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplexer_worktree::FakeGit;

    fn porcelain_locked() -> &'static str {
        "worktree /repo\nHEAD abc123\nbranch refs/heads/main\nlocked\n"
    }

    #[test]
    fn lists_scripted_porcelain() {
        let git = FakeGit::new();
        git.push(Ok(porcelain_locked().into()));
        let catalog = WorktreeService::new(git);
        let trees = catalog.list_worktrees("/repo").expect("list");
        assert_eq!(
            trees,
            vec![WorktreeInfo {
                path: "/repo".into(),
                head: Some("abc123".into()),
                branch: Some("refs/heads/main".into()),
                detached: false,
                locked: true,
                prunable: false,
            }]
        );
    }

    #[test]
    fn lists_detached_and_prunable() {
        let git = FakeGit::new();
        git.push(Ok("worktree /det\nHEAD dead\ndetached\nprunable\n".into()));
        let trees = WorktreeService::new(git)
            .list_worktrees("/det")
            .expect("list");
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].path, "/det");
        assert_eq!(trees[0].head.as_deref(), Some("dead"));
        assert!(trees[0].branch.is_none());
        assert!(trees[0].detached);
        assert!(!trees[0].locked);
        assert!(trees[0].prunable);
    }

    #[test]
    fn exhausted_fake_git_is_provider_error() {
        let catalog = WorktreeService::new(FakeGit::new());
        let err = catalog.list_worktrees("/repo").unwrap_err();
        assert!(matches!(
            err,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::ProviderError && message.contains("no scripted response")
        ));
    }

    #[test]
    fn from_copies_all_flags() {
        let info = WorktreeInfo::from(Worktree {
            path: "/p".into(),
            head: Some("h".into()),
            branch: Some("b".into()),
            detached: true,
            locked: true,
            prunable: true,
        });
        assert_eq!(info.path, "/p");
        assert_eq!(info.head.as_deref(), Some("h"));
        assert_eq!(info.branch.as_deref(), Some("b"));
        assert!(info.detached && info.locked && info.prunable);
    }
}
