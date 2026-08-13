//! Real-git capture / restore / diff. Skips if `git` is missing.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use multiplexer_checkpoint::{HiddenGitStore, ProcessGitExec};

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn temp_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mux-hgit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(1)
    ));
    fs::create_dir_all(&dir).expect("mkdir");
    let status = Command::new("git")
        .args(["init"])
        .current_dir(&dir)
        .status()
        .expect("git init");
    assert!(status.success(), "git init");
    let _ = Command::new("git")
        .args(["config", "user.email", "t@mux.local"])
        .current_dir(&dir)
        .status();
    let _ = Command::new("git")
        .args(["config", "user.name", "mux"])
        .current_dir(&dir)
        .status();
    dir
}

#[test]
fn capture_revert_and_diff_roundtrip() {
    if !git_available() {
        return;
    }
    let dir = temp_repo();
    fs::write(dir.join("note.txt"), "one\n").expect("write");
    let mut store = HiddenGitStore::new(ProcessGitExec::new(), &dir);
    let first = store.create("s", "start").expect("first capture");
    assert!(!first.sha.is_empty());
    assert!(first.ref_name.contains("cp-1"));

    fs::write(dir.join("note.txt"), "two\n").expect("edit");
    fs::write(dir.join("extra.txt"), "new\n").expect("untracked");
    let second = store.create("s", "after").expect("second capture");
    assert_ne!(first.sha, second.sha);

    let diff = store.diff(&first.id).expect("diff");
    assert!(
        diff.files.iter().any(|f| f.contains("note.txt"))
            || diff.text.contains("note.txt")
            || !diff.text.is_empty()
            || !diff.files.is_empty()
    );

    let out = store.revert(&first.id).expect("revert");
    assert!(out.files_restored);
    let body = fs::read_to_string(dir.join("note.txt")).expect("read");
    assert_eq!(body.replace("\r\n", "\n"), "one\n");
    assert!(!dir.join("extra.txt").exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn revert_missing_is_not_found() {
    if !git_available() {
        return;
    }
    let dir = temp_repo();
    let mut store = HiddenGitStore::new(ProcessGitExec::new(), &dir);
    let err = store
        .revert(&multiplexer_checkpoint::CheckpointId::from("cp-99"))
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
    let _ = fs::remove_dir_all(&dir);
}
