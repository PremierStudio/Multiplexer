//! Injected git runner and worktree service (list / add / remove / reminder).

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::porcelain::{find_by_branch, parse_porcelain, PorcelainError, Worktree};

/// Failures from [`WorktreeService`] or a [`GitRunner`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorktreeError {
    /// `git status --porcelain` was non-empty and `force` was false.
    #[error("worktree is dirty: {}", .0.display())]
    Dirty(PathBuf),
    /// The runner failed, or [`FakeGit`] had no scripted response left.
    #[error("{0}")]
    Git(String),
    #[error(transparent)]
    Porcelain(#[from] PorcelainError),
}

/// Runs git in a working directory and returns stdout.
pub trait GitRunner {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<String, WorktreeError>;
}

/// One recorded [`GitRunner::run`] invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCall {
    pub cwd: PathBuf,
    pub args: Vec<String>,
}

/// Scripted git for unit tests. Never touches the process table.
#[derive(Debug, Default)]
pub struct FakeGit {
    calls: RefCell<Vec<GitCall>>,
    responses: RefCell<VecDeque<Result<String, WorktreeError>>>,
}

impl FakeGit {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue the next `run` result (FIFO).
    pub fn push(&self, result: Result<String, WorktreeError>) {
        self.responses.borrow_mut().push_back(result);
    }

    pub fn calls(&self) -> Vec<GitCall> {
        self.calls.borrow().clone()
    }
}

/// Real `git` executable on PATH (or a configured program).
#[derive(Debug, Clone)]
pub struct ProcessGit {
    program: PathBuf,
}

impl ProcessGit {
    pub fn new() -> Self {
        Self {
            program: PathBuf::from("git"),
        }
    }

    pub fn program(&self) -> &Path {
        &self.program
    }
}

impl Default for ProcessGit {
    fn default() -> Self {
        Self::new()
    }
}

impl GitRunner for ProcessGit {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<String, WorktreeError> {
        let output = std::process::Command::new(&self.program)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|err| WorktreeError::Git(format!("spawn git: {err}")))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(WorktreeError::Git(err.trim().to_owned()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

impl GitRunner for FakeGit {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<String, WorktreeError> {
        self.calls.borrow_mut().push(GitCall {
            cwd: cwd.to_path_buf(),
            args: args.iter().map(|s| (*s).to_string()).collect(),
        });
        self.responses
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Err(WorktreeError::Git("fake git: no scripted response".into())))
    }
}

/// Worktree operations over an injected [`GitRunner`].
#[derive(Debug)]
pub struct WorktreeService<R> {
    runner: R,
}

impl<R: GitRunner> WorktreeService<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    pub fn runner(&self) -> &R {
        &self.runner
    }

    /// `git worktree list --porcelain` in `repo`, parsed.
    pub fn list(&self, repo: &Path) -> Result<Vec<Worktree>, WorktreeError> {
        let out = self
            .runner
            .run(repo, &["worktree", "list", "--porcelain"])?;
        Ok(parse_porcelain(&out)?)
    }

    /// Worktrees in `repo` already on `branch` (short or `refs/heads/` form).
    pub fn find_existing(&self, repo: &Path, branch: &str) -> Result<Vec<Worktree>, WorktreeError> {
        let trees = self.list(repo)?;
        Ok(find_by_branch(&trees, branch)
            .into_iter()
            .cloned()
            .collect())
    }

    /// First worktree already on `branch`, if any (pre-existing reminder).
    pub fn reminder(&self, repo: &Path, branch: &str) -> Result<Option<Worktree>, WorktreeError> {
        Ok(self.find_existing(repo, branch)?.into_iter().next())
    }

    /// `git worktree add` (`-b` when `create_branch`).
    pub fn add(
        &self,
        repo: &Path,
        path: &Path,
        branch: &str,
        create_branch: bool,
    ) -> Result<(), WorktreeError> {
        let path_arg = path_arg(path);
        let args: Vec<&str> = if create_branch {
            vec!["worktree", "add", "-b", branch, path_arg.as_str()]
        } else {
            vec!["worktree", "add", path_arg.as_str(), branch]
        };
        self.runner.run(repo, &args).map(|_| ())
    }

    /// Refuse a dirty tree unless `force`, then `git worktree remove` (`-f` if `force`).
    pub fn remove(&self, repo: &Path, path: &Path, force: bool) -> Result<(), WorktreeError> {
        let status = self.runner.run(path, &["status", "--porcelain"])?;
        if status_is_dirty(&status) && !force {
            return Err(WorktreeError::Dirty(path.to_path_buf()));
        }
        let path_arg = path_arg(path);
        let args: Vec<&str> = if force {
            vec!["worktree", "remove", "-f", path_arg.as_str()]
        } else {
            vec!["worktree", "remove", path_arg.as_str()]
        };
        self.runner.run(repo, &args).map(|_| ())
    }
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn status_is_dirty(status: &str) -> bool {
    status.lines().any(|line| !line.trim().is_empty())
}
