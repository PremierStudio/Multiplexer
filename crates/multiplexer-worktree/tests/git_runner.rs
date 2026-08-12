//! `WorktreeService` tests against `FakeGit` only (no real git).

use std::path::{Path, PathBuf};

use multiplexer_worktree::{
    FakeGit, GitCall, GitRunner, PorcelainError, Worktree, WorktreeError, WorktreeService,
};

fn repo() -> PathBuf {
    PathBuf::from("/repo")
}

fn link() -> PathBuf {
    PathBuf::from("/repo/feat")
}

fn call(cwd: &Path, args: &[&str]) -> GitCall {
    GitCall {
        cwd: cwd.to_path_buf(),
        args: args.iter().map(|s| (*s).to_string()).collect(),
    }
}

fn tree(path: &str, branch: Option<&str>) -> Worktree {
    Worktree {
        path: path.to_string(),
        head: Some("aaaa".to_string()),
        branch: branch.map(str::to_string),
        detached: branch.is_none(),
        locked: false,
        prunable: false,
    }
}

fn fleet_porcelain() -> &'static str {
    "\
worktree /repo
HEAD aaaa
branch refs/heads/main

worktree /repo/feat
HEAD aaaa
branch refs/heads/feat

worktree /repo/feat-short
HEAD aaaa
branch feat
"
}

fn service_with(
    results: impl IntoIterator<Item = Result<String, WorktreeError>>,
) -> WorktreeService<FakeGit> {
    let git = FakeGit::new();
    for result in results {
        git.push(result);
    }
    WorktreeService::new(git)
}

#[test]
fn fake_git_is_fifo_records_calls_and_exhausts() {
    let git = FakeGit::new();
    git.push(Ok("one".into()));
    git.push(Ok("two".into()));
    git.push(Err(WorktreeError::Git("boom".into())));
    let cwd = Path::new("/x");
    assert_eq!(git.run(cwd, &["a"]).unwrap(), "one");
    assert_eq!(git.run(cwd, &["b", "c"]).unwrap(), "two");
    assert_eq!(
        git.run(cwd, &["d"]).unwrap_err(),
        WorktreeError::Git("boom".into())
    );
    assert_eq!(
        git.calls(),
        vec![call(cwd, &["a"]), call(cwd, &["b", "c"]), call(cwd, &["d"])]
    );
    assert!(matches!(
        git.run(cwd, &["e"]),
        Err(WorktreeError::Git(msg)) if msg.contains("no scripted response")
    ));
    assert_eq!(git.calls().len(), 4);
}

#[test]
fn list_parses_porcelain_and_uses_exact_argv() {
    let svc = service_with([Ok(fleet_porcelain().into())]);
    let trees = svc.list(&repo()).unwrap();
    assert_eq!(trees.len(), 3);
    assert_eq!(trees[0].path, "/repo");
    assert_eq!(trees[1].branch.as_deref(), Some("refs/heads/feat"));
    assert_eq!(trees[2].branch.as_deref(), Some("feat"));
    assert_eq!(
        svc.runner().calls(),
        vec![call(&repo(), &["worktree", "list", "--porcelain"])]
    );
}

#[test]
fn list_propagates_git_and_parse_errors() {
    let git_err = WorktreeError::Git("denied".into());
    let svc = service_with([Err(WorktreeError::Git("denied".into()))]);
    assert_eq!(svc.list(&repo()).unwrap_err(), git_err);

    let svc = service_with([Ok("garbage line\n".into())]);
    assert_eq!(
        svc.list(&repo()).unwrap_err(),
        WorktreeError::Porcelain(PorcelainError::UnexpectedLine("garbage line".into()))
    );
}

#[test]
fn find_existing_keeps_only_matching_branch_forms() {
    let svc = service_with([Ok(fleet_porcelain().into())]);
    let found = svc.find_existing(&repo(), "feat").unwrap();
    let paths: Vec<&str> = found.iter().map(|t| t.path.as_str()).collect();
    assert_eq!(paths, vec!["/repo/feat", "/repo/feat-short"]);
    assert_eq!(found[0], tree("/repo/feat", Some("refs/heads/feat")));
    assert_eq!(found[1], tree("/repo/feat-short", Some("feat")));
}

#[test]
fn find_existing_empty_when_branch_absent() {
    let svc = service_with([Ok(fleet_porcelain().into())]);
    assert!(svc.find_existing(&repo(), "missing").unwrap().is_empty());
}

#[test]
fn reminder_returns_first_match_only() {
    let svc = service_with([Ok(fleet_porcelain().into())]);
    let hit = svc.reminder(&repo(), "feat").unwrap();
    assert_eq!(hit, Some(tree("/repo/feat", Some("refs/heads/feat"))));
}

#[test]
fn reminder_none_when_branch_absent() {
    let svc = service_with([Ok(fleet_porcelain().into())]);
    assert_eq!(svc.reminder(&repo(), "missing").unwrap(), None);
}

#[test]
fn reminder_propagates_list_errors() {
    let svc = service_with([Err(WorktreeError::Git("no git".into()))]);
    assert_eq!(
        svc.reminder(&repo(), "feat").unwrap_err(),
        WorktreeError::Git("no git".into())
    );
}

#[test]
fn add_create_branch_uses_dash_b_before_path() {
    let svc = service_with([Ok(String::new())]);
    svc.add(&repo(), &link(), "feat", true).unwrap();
    assert_eq!(
        svc.runner().calls(),
        vec![call(
            &repo(),
            &["worktree", "add", "-b", "feat", "/repo/feat"]
        )]
    );
}

#[test]
fn add_existing_branch_has_no_dash_b_and_path_then_branch() {
    let svc = service_with([Ok(String::new())]);
    svc.add(&repo(), &link(), "feat", false).unwrap();
    assert_eq!(
        svc.runner().calls(),
        vec![call(&repo(), &["worktree", "add", "/repo/feat", "feat"])]
    );
}

#[test]
fn add_propagates_runner_error() {
    let svc = service_with([Err(WorktreeError::Git("exists".into()))]);
    assert_eq!(
        svc.add(&repo(), &link(), "feat", true).unwrap_err(),
        WorktreeError::Git("exists".into())
    );
}

#[test]
fn remove_refuses_dirty_without_force_and_skips_remove() {
    for status in [" M foo.rs\n", "?? new.txt\n", "A  added.rs", "D  gone.rs\n"] {
        let svc = service_with([Ok(status.into())]);
        assert_eq!(
            svc.remove(&repo(), &link(), false).unwrap_err(),
            WorktreeError::Dirty(link())
        );
        assert_eq!(
            svc.runner().calls(),
            vec![call(&link(), &["status", "--porcelain"])]
        );
    }
}

#[test]
fn remove_whitespace_only_status_is_clean() {
    for status in ["", "\n", " \n", "\t", "\r\n", "   \n\n"] {
        let svc = service_with([Ok(status.into()), Ok(String::new())]);
        svc.remove(&repo(), &link(), false).unwrap();
        assert_eq!(
            svc.runner().calls(),
            vec![
                call(&link(), &["status", "--porcelain"]),
                call(&repo(), &["worktree", "remove", "/repo/feat"]),
            ]
        );
    }
}

#[test]
fn remove_dirty_with_force_uses_dash_f() {
    let svc = service_with([Ok(" M foo.rs\n".into()), Ok(String::new())]);
    svc.remove(&repo(), &link(), true).unwrap();
    assert_eq!(
        svc.runner().calls(),
        vec![
            call(&link(), &["status", "--porcelain"]),
            call(&repo(), &["worktree", "remove", "-f", "/repo/feat"]),
        ]
    );
}

#[test]
fn remove_clean_with_force_still_passes_dash_f() {
    let svc = service_with([Ok(String::new()), Ok(String::new())]);
    svc.remove(&repo(), &link(), true).unwrap();
    assert_eq!(
        svc.runner().calls(),
        vec![
            call(&link(), &["status", "--porcelain"]),
            call(&repo(), &["worktree", "remove", "-f", "/repo/feat"]),
        ]
    );
}

#[test]
fn remove_status_error_skips_remove() {
    let svc = service_with([Err(WorktreeError::Git("status failed".into()))]);
    assert_eq!(
        svc.remove(&repo(), &link(), false).unwrap_err(),
        WorktreeError::Git("status failed".into())
    );
    assert_eq!(
        svc.runner().calls(),
        vec![call(&link(), &["status", "--porcelain"])]
    );
}

#[test]
fn remove_command_error_propagates() {
    let svc = service_with([Ok(String::new()), Err(WorktreeError::Git("locked".into()))]);
    assert_eq!(
        svc.remove(&repo(), &link(), false).unwrap_err(),
        WorktreeError::Git("locked".into())
    );
    assert_eq!(svc.runner().calls().len(), 2);
}

#[test]
fn worktree_error_display() {
    let dirty = WorktreeError::Dirty(link());
    assert!(dirty.to_string().contains("worktree is dirty"));
    assert!(dirty.to_string().contains("feat"));
    assert_eq!(WorktreeError::Git("boom".into()).to_string(), "boom");
    assert_eq!(
        WorktreeError::Porcelain(PorcelainError::MissingPath).to_string(),
        "worktree entry missing path"
    );
}
