//! Optional git catalog used by `git.worktrees` and `git.worktree.create`.

use std::path::Path;

use multiplexer_wire::error::AppErrorKind;
use multiplexer_worktree::{GitRunner, Worktree, WorktreeError, WorktreeService};
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

/// Lists and creates worktrees for a working directory. Tests inject [`WorktreeService<FakeGit>`].
pub trait GitCatalog: Send {
    fn list_worktrees(&self, cwd: &str) -> Result<Vec<WorktreeInfo>, BackendError>;
    fn create_worktree(
        &self,
        cwd: &str,
        path: &str,
        branch: &str,
        create_branch: bool,
    ) -> Result<WorktreeInfo, BackendError>;
}

impl<R: GitRunner + Send> GitCatalog for WorktreeService<R> {
    fn list_worktrees(&self, cwd: &str) -> Result<Vec<WorktreeInfo>, BackendError> {
        self.list(Path::new(cwd))
            .map(|trees| trees.into_iter().map(WorktreeInfo::from).collect())
            .map_err(map_worktree_err)
    }

    fn create_worktree(
        &self,
        cwd: &str,
        path: &str,
        branch: &str,
        create_branch: bool,
    ) -> Result<WorktreeInfo, BackendError> {
        self.add(Path::new(cwd), Path::new(path), branch, create_branch)
            .map_err(map_worktree_err)?;
        let fallback = WorktreeInfo {
            path: path.to_owned(),
            branch: Some(branch.to_owned()),
            ..WorktreeInfo::default()
        };
        let Ok(trees) = self.list(Path::new(cwd)) else {
            return Ok(fallback);
        };
        Ok(trees
            .into_iter()
            .map(WorktreeInfo::from)
            .find(|tree| paths_match(&tree.path, path))
            .unwrap_or(fallback))
    }
}

fn map_worktree_err(err: WorktreeError) -> BackendError {
    BackendError::Provider {
        kind: AppErrorKind::ProviderError,
        message: err.to_string(),
    }
}

fn paths_match(listed: &str, requested: &str) -> bool {
    listed == requested || Path::new(listed) == Path::new(requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use multiplexer_worktree::{FakeGit, WorktreeError};

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
    fn create_worktree_calls_add() {
        let git = FakeGit::new();
        git.push(Ok(String::new()));
        git.push(Ok(
            "worktree /repo\nHEAD abc123\nbranch refs/heads/main\n\n\
             worktree /wt\nHEAD def456\nbranch refs/heads/feat\n"
                .into(),
        ));
        let catalog = WorktreeService::new(git);
        let info = catalog
            .create_worktree("/repo", "/wt", "feat", false)
            .expect("create");
        assert_eq!(
            info,
            WorktreeInfo {
                path: "/wt".into(),
                head: Some("def456".into()),
                branch: Some("refs/heads/feat".into()),
                detached: false,
                locked: false,
                prunable: false,
            }
        );
        let calls = catalog.runner().calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].args,
            ["worktree", "add", "/wt", "feat"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            calls[1].args,
            ["worktree", "list", "--porcelain"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn create_worktree_maps_runner_error() {
        let git = FakeGit::new();
        git.push(Err(WorktreeError::Git("add failed".into())));
        let err = WorktreeService::new(git)
            .create_worktree("/repo", "/wt", "feat", false)
            .unwrap_err();
        assert!(matches!(
            err,
            BackendError::Provider { kind, ref message }
                if kind == AppErrorKind::ProviderError && message.contains("add failed")
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
