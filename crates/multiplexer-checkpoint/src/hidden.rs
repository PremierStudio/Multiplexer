//! Hidden git refs under `refs/multiplexer/checkpoints/<id>`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Checkpoint, CheckpointError, CheckpointId, CheckpointStore};

/// Runs git in a working tree. Extra author env is applied by [`ProcessGitExec`].
pub trait GitExec {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<GitOut, CheckpointError>;
}

/// One git invocation result. Non-zero is not always a failure (`diff --quiet`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOut {
    pub stdout: String,
    pub code: i32,
}

impl GitOut {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            code: 0,
        }
    }
}

/// Real `git` with checkpoint author identity.
#[derive(Debug, Clone, Default)]
pub struct ProcessGitExec;

impl ProcessGitExec {
    pub fn new() -> Self {
        Self
    }
}

impl GitExec for ProcessGitExec {
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<GitOut, CheckpointError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_AUTHOR_NAME", "multiplexer")
            .env("GIT_AUTHOR_EMAIL", "checkpoints@multiplexer.local")
            .env("GIT_COMMITTER_NAME", "multiplexer")
            .env("GIT_COMMITTER_EMAIL", "checkpoints@multiplexer.local")
            .output()
            .map_err(|e| CheckpointError::Git(format!("spawn git: {e}")))?;
        Ok(GitOut {
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            code: output.status.code().unwrap_or(1),
        })
    }
}

/// Scripted git for unit tests.
#[derive(Debug, Default)]
pub struct FakeGitExec {
    calls: RefCell<Vec<Vec<String>>>,
    responses: RefCell<VecDeque<Result<GitOut, CheckpointError>>>,
}

impl FakeGitExec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, result: Result<GitOut, CheckpointError>) {
        self.responses.borrow_mut().push_back(result);
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }
}

impl GitExec for FakeGitExec {
    fn run(&self, _cwd: &Path, args: &[&str]) -> Result<GitOut, CheckpointError> {
        self.calls
            .borrow_mut()
            .push(args.iter().map(|s| (*s).to_string()).collect());
        self.responses.borrow_mut().pop_front().unwrap_or_else(|| {
            Err(CheckpointError::Git(
                "fake git: no scripted response".into(),
            ))
        })
    }
}

/// Diff between a checkpoint and the current tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointDiff {
    pub checkpoint_id: String,
    pub sha: String,
    pub text: String,
    pub files: Vec<String>,
}

/// RAM catalog plus hidden-ref capture/restore.
#[derive(Debug)]
pub struct HiddenGitStore<R> {
    inner: CheckpointStore,
    git: R,
    cwd: PathBuf,
}

impl<R: GitExec> HiddenGitStore<R> {
    pub fn new(git: R, cwd: impl Into<PathBuf>) -> Self {
        Self {
            inner: CheckpointStore::new(),
            git,
            cwd: cwd.into(),
        }
    }

    pub fn inner(&self) -> &CheckpointStore {
        &self.inner
    }

    pub fn cwd(&self) -> &Path {
        &self.cwd
    }

    pub fn git(&self) -> &R {
        &self.git
    }

    pub fn list(&self, session_id: &str) -> Vec<Checkpoint> {
        self.inner.list(session_id)
    }

    pub fn current(&self, session_id: &str) -> Option<CheckpointId> {
        self.inner.current(session_id)
    }

    /// Snapshot the worktree into `refs/multiplexer/checkpoints/<id>`.
    pub fn create(&mut self, session_id: &str, label: &str) -> Result<Checkpoint, CheckpointError> {
        let id = self.inner.peek_next_id();
        let cap = capture(&self.git, &self.cwd, id.as_str(), label)?;
        let mut row = self.inner.create(session_id, label);
        debug_assert_eq!(row.id, id);
        self.inner.attach_git(&row.id, &cap.sha, &cap.ref_name);
        row.sha = cap.sha;
        row.ref_name = cap.ref_name;
        Ok(row)
    }

    /// Move the pointer and hard-reset the worktree when a SHA exists.
    pub fn revert(&mut self, id: &CheckpointId) -> Result<RevertOutcome, CheckpointError> {
        let cp = self.inner.revert(id)?;
        if cp.sha.is_empty() && cp.ref_name.is_empty() {
            return Ok(RevertOutcome {
                checkpoint: cp,
                files_restored: false,
            });
        }
        restore(&self.git, &self.cwd, &cp)?;
        Ok(RevertOutcome {
            checkpoint: cp,
            files_restored: true,
        })
    }

    pub fn diff(&self, id: &CheckpointId) -> Result<CheckpointDiff, CheckpointError> {
        let cp = self
            .inner
            .get(id)
            .ok_or_else(|| CheckpointError::NotFound(id.clone()))?;
        if cp.sha.is_empty() {
            return Err(CheckpointError::Git(
                "checkpoint has no hidden-git sha".into(),
            ));
        }
        diff_against(&self.git, &self.cwd, &cp)
    }
}

/// Result of [`HiddenGitStore::revert`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevertOutcome {
    pub checkpoint: Checkpoint,
    pub files_restored: bool,
}

struct Capture {
    sha: String,
    ref_name: String,
}

pub fn ref_for(id: &str) -> String {
    format!("refs/multiplexer/checkpoints/{id}")
}

fn capture<R: GitExec>(
    git: &R,
    cwd: &Path,
    id: &str,
    label: &str,
) -> Result<Capture, CheckpointError> {
    require_ok(git.run(cwd, &["add", "-A"])?, "git add -A")?;
    let tree = require_ok(git.run(cwd, &["write-tree"])?, "git write-tree")?;
    if tree.stdout.is_empty() {
        return Err(CheckpointError::Git("write-tree produced empty sha".into()));
    }
    let parent = git.run(cwd, &["rev-parse", "HEAD"]).ok();
    let msg = format!("multiplexer: checkpoint {id} {label}");
    let commit = if let Some(p) = parent.filter(|o| o.code == 0 && !o.stdout.is_empty()) {
        require_ok(
            git.run(
                cwd,
                &["commit-tree", &tree.stdout, "-p", &p.stdout, "-m", &msg],
            )?,
            "git commit-tree",
        )?
    } else {
        require_ok(
            git.run(cwd, &["commit-tree", &tree.stdout, "-m", &msg])?,
            "git commit-tree",
        )?
    };
    if commit.stdout.is_empty() {
        return Err(CheckpointError::Git(
            "commit-tree produced empty sha".into(),
        ));
    }
    let ref_name = ref_for(id);
    require_ok(
        git.run(cwd, &["update-ref", &ref_name, &commit.stdout])?,
        "git update-ref",
    )?;
    let _ = git.run(cwd, &["reset", "-q"]);
    Ok(Capture {
        sha: commit.stdout,
        ref_name,
    })
}

fn restore<R: GitExec>(git: &R, cwd: &Path, cp: &Checkpoint) -> Result<(), CheckpointError> {
    let target = if !cp.sha.is_empty() {
        cp.sha.as_str()
    } else {
        cp.ref_name.as_str()
    };
    require_ok(
        git.run(cwd, &["reset", "--hard", target])?,
        "git reset --hard",
    )?;
    require_ok(git.run(cwd, &["clean", "-fd"])?, "git clean -fd")?;
    Ok(())
}

fn diff_against<R: GitExec>(
    git: &R,
    cwd: &Path,
    cp: &Checkpoint,
) -> Result<CheckpointDiff, CheckpointError> {
    let names = git.run(cwd, &["diff", "--name-only", &cp.sha])?;
    if names.code != 0 && names.code != 1 {
        return Err(CheckpointError::Git(format!(
            "git diff --name-only exited {}",
            names.code
        )));
    }
    let text = git.run(cwd, &["diff", &cp.sha])?;
    if text.code != 0 && text.code != 1 {
        return Err(CheckpointError::Git(format!(
            "git diff exited {}",
            text.code
        )));
    }
    let files = names
        .stdout
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    Ok(CheckpointDiff {
        checkpoint_id: cp.id.to_string(),
        sha: cp.sha.clone(),
        text: text.stdout,
        files,
    })
}

fn require_ok(out: GitOut, what: &str) -> Result<GitOut, CheckpointError> {
    if out.code != 0 {
        Err(CheckpointError::Git(format!("{what} exited {}", out.code)))
    } else {
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scripted_capture(fake: &FakeGitExec) {
        fake.push(Ok(GitOut::ok("")));
        fake.push(Ok(GitOut::ok("tree111")));
        fake.push(Ok(GitOut::ok("parent222")));
        fake.push(Ok(GitOut::ok("sha333")));
        fake.push(Ok(GitOut::ok("")));
        fake.push(Ok(GitOut::ok("")));
    }

    #[test]
    fn create_records_sha_and_ref() {
        let fake = FakeGitExec::new();
        scripted_capture(&fake);
        let mut store = HiddenGitStore::new(fake, ".");
        let cp = store.create("s", "manual").expect("create");
        assert_eq!(cp.id.as_str(), "cp-1");
        assert_eq!(cp.sha, "sha333");
        assert_eq!(cp.ref_name, "refs/multiplexer/checkpoints/cp-1");
        assert_eq!(store.current("s").unwrap().as_str(), "cp-1");
        assert_eq!(store.list("s").len(), 1);
        assert_eq!(store.inner().list("s").len(), 1);
        assert!(!store.inner().list("s")[0].sha.is_empty());
        let calls = store.git().calls();
        assert_eq!(calls[0], vec!["add", "-A"]);
        assert_eq!(calls[1], vec!["write-tree"]);
        assert_eq!(calls[2], vec!["rev-parse", "HEAD"]);
        assert_eq!(calls[3][0], "commit-tree");
        assert!(calls[3].contains(&"tree111".to_owned()));
        assert!(calls[3].contains(&"parent222".to_owned()));
        assert_eq!(
            calls[4],
            vec!["update-ref", "refs/multiplexer/checkpoints/cp-1", "sha333"]
        );
    }

    #[test]
    fn create_without_parent_omits_dash_p() {
        let fake = FakeGitExec::new();
        fake.push(Ok(GitOut::ok("")));
        fake.push(Ok(GitOut::ok("tree0")));
        fake.push(Ok(GitOut {
            stdout: String::new(),
            code: 128,
        }));
        fake.push(Ok(GitOut::ok("sha0")));
        fake.push(Ok(GitOut::ok("")));
        fake.push(Ok(GitOut::ok("")));
        let mut store = HiddenGitStore::new(fake, ".");
        let cp = store.create("s", "start").expect("create");
        assert_eq!(cp.sha, "sha0");
        let commit = &store.git().calls()[3];
        assert!(!commit.iter().any(|a| a == "-p"));
    }

    #[test]
    fn create_fails_when_write_tree_empty() {
        let fake = FakeGitExec::new();
        fake.push(Ok(GitOut::ok("")));
        fake.push(Ok(GitOut::ok("")));
        let mut store = HiddenGitStore::new(fake, ".");
        let err = store.create("s", "x").unwrap_err();
        assert!(matches!(err, CheckpointError::Git(_)));
        assert!(store.list("s").is_empty());
    }

    #[test]
    fn revert_without_sha_is_pointer_only() {
        let mut store = HiddenGitStore::new(FakeGitExec::new(), ".");
        let row = store.inner.create("s", "ram");
        let out = store.revert(&row.id).expect("revert");
        assert!(!out.files_restored);
        assert_eq!(out.checkpoint.id, row.id);
    }

    #[test]
    fn revert_with_sha_resets_hard() {
        let fake = FakeGitExec::new();
        scripted_capture(&fake);
        fake.push(Ok(GitOut::ok("HEAD is now at sha333")));
        fake.push(Ok(GitOut::ok("")));
        let mut store = HiddenGitStore::new(fake, ".");
        let cp = store.create("s", "snap").expect("create");
        let out = store.revert(&cp.id).expect("revert");
        assert!(out.files_restored);
        let calls = store.git().calls();
        let reset = calls.iter().rev().nth(1).expect("reset");
        assert_eq!(reset[..2], ["reset", "--hard"]);
        assert_eq!(reset[2], "sha333");
        assert_eq!(calls.last().unwrap(), &["clean", "-fd"]);
    }

    #[test]
    fn revert_with_sha_only_still_restores() {
        let fake = FakeGitExec::new();
        fake.push(Ok(GitOut::ok("")));
        fake.push(Ok(GitOut::ok("")));
        let mut store = HiddenGitStore::new(fake, ".");
        let row = store.inner.create("s", "ram");
        assert!(store.inner.attach_git(&row.id, "onlysha", ""));
        let out = store.revert(&row.id).expect("revert");
        assert!(out.files_restored);
        assert_eq!(store.git().calls()[0], vec!["reset", "--hard", "onlysha"]);
    }

    #[test]
    fn revert_unknown_is_not_found() {
        let mut store = HiddenGitStore::new(FakeGitExec::new(), ".");
        let err = store
            .revert(&CheckpointId::from("cp-9"))
            .expect_err("missing");
        assert!(matches!(err, CheckpointError::NotFound(_)));
    }

    #[test]
    fn diff_lists_files_and_text() {
        let fake = FakeGitExec::new();
        scripted_capture(&fake);
        fake.push(Ok(GitOut {
            stdout: "src/lib.rs\nREADME.md".into(),
            code: 1,
        }));
        fake.push(Ok(GitOut {
            stdout: "diff --git a/src/lib.rs".into(),
            code: 1,
        }));
        let mut store = HiddenGitStore::new(fake, ".");
        let cp = store.create("s", "snap").expect("create");
        let d = store.diff(&cp.id).expect("diff");
        assert_eq!(d.files, vec!["src/lib.rs", "README.md"]);
        assert!(d.text.contains("lib.rs"));
        assert_eq!(d.sha, "sha333");
    }

    #[test]
    fn diff_missing_sha_errors() {
        let mut store = HiddenGitStore::new(FakeGitExec::new(), ".");
        let row = store.inner.create("s", "ram");
        let err = store.diff(&row.id).unwrap_err();
        assert!(matches!(err, CheckpointError::Git(_)));
    }

    #[test]
    fn ref_for_uses_hidden_namespace() {
        assert_eq!(ref_for("cp-4"), "refs/multiplexer/checkpoints/cp-4");
        assert!(!ref_for("cp-4").contains("refs/heads"));
    }

    #[test]
    fn git_out_ok_is_zero() {
        assert_eq!(GitOut::ok("x").code, 0);
        assert_ne!(GitOut::ok("x").stdout, "");
    }
}
