use std::path::Path;

use multiplexer_worktree::{GitRunner, ProcessGit};

#[test]
fn process_git_reports_version() {
    let git = ProcessGit::new();
    assert_eq!(git.program(), Path::new("git"));
    let out = git
        .run(Path::new("."), &["--version"])
        .expect("git --version");
    assert!(
        out.to_ascii_lowercase().contains("git version"),
        "unexpected git --version output: {out}"
    );
}

#[test]
fn process_git_lists_this_repo_worktrees() {
    let git = ProcessGit::new();
    let out = git
        .run(Path::new("."), &["worktree", "list", "--porcelain"])
        .expect("git worktree list");
    assert!(
        out.contains("worktree "),
        "expected porcelain worktree line, got: {out}"
    );
}
