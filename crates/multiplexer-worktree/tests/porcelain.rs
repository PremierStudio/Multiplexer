//! Integration tests and proptests for the porcelain parser.

use multiplexer_worktree::{find_by_branch, parse_porcelain, PorcelainError, Worktree};
use proptest::prelude::*;

/// Serialize a worktree back to porcelain text (mirror of the crate's helper,
/// exercised here as a round-trip oracle).
fn to_porcelain(trees: &[Worktree]) -> String {
    let mut out = String::new();
    for (i, tree) in trees.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str("worktree ");
        out.push_str(&tree.path);
        out.push('\n');
        if let Some(head) = &tree.head {
            out.push_str("HEAD ");
            out.push_str(head);
            out.push('\n');
        }
        match &tree.branch {
            Some(branch) => {
                out.push_str("branch ");
                out.push_str(branch);
                out.push('\n');
            }
            None => {
                out.push_str("detached\n");
            }
        }
        if tree.locked {
            out.push_str("locked\n");
        }
        if tree.prunable {
            out.push_str("prunable\n");
        }
    }
    out
}

#[test]
fn integration_parse_and_find() {
    let input = "\
worktree /repo
HEAD aaaa
branch refs/heads/main

worktree /repo/feat
HEAD bbbb
branch refs/heads/feature

worktree /repo/det
HEAD cccc
detached
";
    let trees = parse_porcelain(input).unwrap();
    assert_eq!(trees.len(), 3);
    assert_eq!(
        find_by_branch(&trees, "feature")
            .iter()
            .map(|t| t.path.as_str())
            .collect::<Vec<_>>(),
        vec!["/repo/feat"]
    );
    assert!(trees[2].detached);
    assert_eq!(trees[2].branch, None);
}

#[test]
fn missing_path_line_errors() {
    // A non-empty line before any `worktree` line is an unexpected line.
    assert_eq!(
        parse_porcelain("locked\n").unwrap_err(),
        PorcelainError::UnexpectedLine("locked".to_string())
    );
}

#[test]
fn worktree_without_path_is_missing_path() {
    assert_eq!(
        parse_porcelain("worktree\n").unwrap_err(),
        PorcelainError::MissingPath
    );
}

#[test]
fn porcelain_error_display() {
    assert_eq!(
        PorcelainError::UnexpectedLine("x".into()).to_string(),
        "expected worktree line, got: x"
    );
    assert_eq!(
        PorcelainError::MissingPath.to_string(),
        "worktree entry missing path"
    );
}

proptest! {
    #[test]
    fn roundtrip_porcelain(
        entries in proptest::collection::vec(
            (
                "[a-z0-9/]{1,20}",
                proptest::option::of(proptest::bool::ANY),
                proptest::bool::ANY,
                proptest::bool::ANY,
            ),
            0..10,
        )
    ) {
        let trees: Vec<Worktree> = entries
            .into_iter()
            .map(|(path, is_linked, locked, prunable)| {
                let branch = if is_linked.unwrap_or(false) {
                    Some(format!("refs/heads/{}", path.replace('/', "_")))
                } else {
                    None
                };
                let detached = branch.is_none();
                Worktree {
                    path,
                    head: Some("deadbeef".to_string()),
                    branch,
                    detached,
                    locked,
                    prunable,
                }
            })
            .collect();
        let text = to_porcelain(&trees);
        let parsed = parse_porcelain(&text).unwrap();
        assert_eq!(parsed, trees);
    }
}
