//! Parser for the `git worktree list --porcelain` text format.

use thiserror::Error;

/// A single worktree entry parsed from porcelain output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
}

/// Errors produced while parsing porcelain output.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PorcelainError {
    #[error("expected worktree line, got: {0}")]
    UnexpectedLine(String),
    #[error("worktree entry missing path")]
    MissingPath,
}

/// Parse `git worktree list --porcelain` output into worktree records.
///
/// Records are separated by blank lines. The first (and only the first) line of
/// a record begins with `worktree ` and gives the path.
pub fn parse_porcelain(input: &str) -> Result<Vec<Worktree>, PorcelainError> {
    let mut trees = Vec::new();
    let mut current: Option<Worktree> = None;

    let flush = |trees: &mut Vec<Worktree>, current: &mut Option<Worktree>| {
        if let Some(tree) = current.take() {
            trees.push(tree);
        }
    };

    for raw in input.lines() {
        let line = raw.trim_end();
        if line.is_empty() {
            flush(&mut trees, &mut current);
            continue;
        }
        if line == "worktree" || line.starts_with("worktree ") {
            let path = line.strip_prefix("worktree ").unwrap_or("");
            let tree = Worktree {
                path: path.to_string(),
                head: None,
                branch: None,
                detached: false,
                locked: false,
                prunable: false,
            };
            let prev = current.replace(tree);
            debug_assert!(prev.is_none(), "record with multiple worktree lines");
            continue;
        }
        let tree = match current.as_mut() {
            Some(tree) => tree,
            None => return Err(PorcelainError::UnexpectedLine(line.to_string())),
        };
        if let Some(head) = line.strip_prefix("HEAD ") {
            tree.head = Some(head.to_string());
        } else if line == "detached" {
            tree.detached = true;
        } else if line == "locked" {
            tree.locked = true;
        } else if line == "prunable" {
            tree.prunable = true;
        } else if line == "bare" {
            // Bare repository marker; no branch is recorded.
        } else if let Some(branch) = line.strip_prefix("branch ") {
            tree.branch = Some(branch.to_string());
        } else {
            return Err(PorcelainError::UnexpectedLine(line.to_string()));
        }
    }
    flush(&mut trees, &mut current);

    for tree in &trees {
        if tree.path.is_empty() {
            return Err(PorcelainError::MissingPath);
        }
    }
    Ok(trees)
}

/// Return worktrees whose branch is `refs/heads/{name}` or `{name}`.
pub fn find_by_branch<'a>(trees: &'a [Worktree], name: &str) -> Vec<&'a Worktree> {
    let full = format!("refs/heads/{name}");
    trees
        .iter()
        .filter(|t| t.branch.as_deref() == Some(name) || t.branch.as_deref() == Some(full.as_str()))
        .collect()
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn empty_input_yields_empty_vec() {
        assert_eq!(parse_porcelain("").unwrap(), Vec::new());
        assert_eq!(parse_porcelain("\n\n").unwrap(), Vec::new());
    }

    #[test]
    fn single_main_worktree() {
        let input = "worktree /repo\nHEAD abc123\nbranch refs/heads/main\n";
        let trees = parse_porcelain(input).unwrap();
        assert_eq!(
            trees,
            vec![Worktree {
                path: "/repo".to_string(),
                head: Some("abc123".to_string()),
                branch: Some("refs/heads/main".to_string()),
                detached: false,
                locked: false,
                prunable: false,
            }]
        );
    }

    #[test]
    fn detached_linked_worktree() {
        let input = "worktree /repo/linked\nHEAD 1234abcd\nbranch refs/heads/feature\n";
        let trees = parse_porcelain(input).unwrap();
        assert_eq!(trees[0].branch.as_deref(), Some("refs/heads/feature"));
        assert!(!trees[0].detached);

        let detached = "worktree /repo/det\ndetached\nHEAD 9999\n";
        let trees = parse_porcelain(detached).unwrap();
        assert!(trees[0].detached);
        assert_eq!(trees[0].branch, None);
        assert_eq!(trees[0].head.as_deref(), Some("9999"));
    }

    #[test]
    fn locked_and_prunable_flags() {
        let input =
            "worktree /repo/other\nHEAD deadbeef\nbranch refs/heads/feature\nlocked\nprunable\n";
        let trees = parse_porcelain(input).unwrap();
        assert_eq!(trees.len(), 1);
        assert!(trees[0].locked);
        assert!(trees[0].prunable);
    }

    #[test]
    fn multiple_records_separated_by_blank_line() {
        let input = "\
worktree /repo/main
HEAD aaaa
branch refs/heads/main

worktree /repo/link
HEAD bbbb
branch refs/heads/dev

worktree /repo/det
HEAD cccc
detached
";
        let trees = parse_porcelain(input).unwrap();
        assert_eq!(trees.len(), 3);
        assert_eq!(trees[0].path, "/repo/main");
        assert_eq!(trees[0].branch.as_deref(), Some("refs/heads/main"));
        assert_eq!(trees[1].path, "/repo/link");
        assert_eq!(trees[1].branch.as_deref(), Some("refs/heads/dev"));
        assert_eq!(trees[2].path, "/repo/det");
        assert!(trees[2].detached);
    }

    #[test]
    fn unexpected_first_line_errors() {
        let err = parse_porcelain("garbage line\n").unwrap_err();
        assert_eq!(
            err,
            PorcelainError::UnexpectedLine("garbage line".to_string())
        );
    }

    #[test]
    fn unexpected_attribute_after_worktree_errors() {
        let err = parse_porcelain("worktree /repo\nunknown-attr\n").unwrap_err();
        assert_eq!(
            err,
            PorcelainError::UnexpectedLine("unknown-attr".to_string())
        );
    }

    #[test]
    fn bare_marker_is_accepted() {
        let trees = parse_porcelain("worktree /repo.git\nbare\nHEAD abc\n").unwrap();
        assert_eq!(trees.len(), 1);
        assert_eq!(trees[0].path, "/repo.git");
        assert_eq!(trees[0].head.as_deref(), Some("abc"));
        assert_eq!(trees[0].branch, None);
        assert!(!trees[0].detached);
    }

    #[test]
    fn worktree_line_without_path_is_missing_path() {
        assert_eq!(
            parse_porcelain("worktree").unwrap_err(),
            PorcelainError::MissingPath
        );
        assert_eq!(
            parse_porcelain("worktree\nHEAD abc\n").unwrap_err(),
            PorcelainError::MissingPath
        );
    }

    #[test]
    fn find_by_branch_matches_full_and_short_name() {
        let trees = vec![
            Worktree {
                path: "/a".into(),
                head: Some("1".into()),
                branch: Some("refs/heads/foo".into()),
                detached: false,
                locked: false,
                prunable: false,
            },
            Worktree {
                path: "/b".into(),
                head: Some("2".into()),
                branch: Some("foo".into()),
                detached: false,
                locked: false,
                prunable: false,
            },
            Worktree {
                path: "/c".into(),
                head: Some("3".into()),
                branch: Some("refs/heads/bar".into()),
                detached: false,
                locked: false,
                prunable: false,
            },
            Worktree {
                path: "/d".into(),
                head: Some("4".into()),
                branch: None,
                detached: true,
                locked: false,
                prunable: false,
            },
        ];
        let found = find_by_branch(&trees, "foo");
        let paths: Vec<&str> = found.iter().map(|t| t.path.as_str()).collect();
        assert_eq!(paths, vec!["/a", "/b"]);
        assert!(find_by_branch(&trees, "bar").len() == 1);
        assert!(find_by_branch(&trees, "nope").is_empty());
    }
}
