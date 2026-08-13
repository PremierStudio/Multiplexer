//! Pre-existing worktree reminder from a parsed fleet.

use crate::porcelain::{find_by_branch, Worktree};

/// A linked worktree the user may want to resume instead of creating another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reminder {
    pub branch: String,
    pub path: String,
}

/// If `current_branch` is already checked out and the repo has more than one
/// worktree, point at the first linked (non-main) match, or the first linked
/// worktree when the only match is the main path.
pub fn reminder_from_list(trees: &[Worktree], current_branch: &str) -> Option<Reminder> {
    if trees.len() <= 1 {
        return None;
    }
    let matches = find_by_branch(trees, current_branch);
    if matches.is_empty() {
        return None;
    }
    let main_path = trees[0].path.as_str();
    let chosen = matches
        .into_iter()
        .find(|tree| tree.path != main_path)
        .unwrap_or(&trees[1]);
    Some(Reminder {
        branch: current_branch.to_string(),
        path: chosen.path.clone(),
    })
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

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

    #[test]
    fn empty_list_is_none() {
        assert_eq!(reminder_from_list(&[], "main"), None);
    }

    #[test]
    fn single_worktree_is_none_even_when_branch_matches() {
        let trees = [tree("/repo", Some("refs/heads/main"))];
        assert_eq!(reminder_from_list(&trees, "main"), None);
        assert_eq!(reminder_from_list(&trees, "refs/heads/main"), None);
    }

    #[test]
    fn no_branch_match_is_none_when_fleet_has_links() {
        let trees = [
            tree("/repo", Some("refs/heads/main")),
            tree("/repo/feat", Some("refs/heads/feat")),
        ];
        assert_eq!(reminder_from_list(&trees, "other"), None);
        assert_eq!(reminder_from_list(&trees, "missing"), None);
    }

    #[test]
    fn linked_match_is_preferred_over_main_path() {
        let trees = [
            tree("/repo", Some("refs/heads/feat")),
            tree("/repo/feat", Some("refs/heads/feat")),
            tree("/repo/feat-2", Some("refs/heads/feat")),
        ];
        assert_eq!(
            reminder_from_list(&trees, "feat"),
            Some(Reminder {
                branch: "feat".into(),
                path: "/repo/feat".into(),
            })
        );
    }

    #[test]
    fn first_linked_match_wins_when_several_match() {
        let trees = [
            tree("/repo", Some("refs/heads/main")),
            tree("/repo/a", Some("refs/heads/feat")),
            tree("/repo/b", Some("refs/heads/feat")),
        ];
        let reminder = reminder_from_list(&trees, "feat").unwrap();
        assert_eq!(reminder.branch, "feat");
        assert_eq!(reminder.path, "/repo/a");
    }

    #[test]
    fn short_and_full_branch_names_match() {
        let short = [
            tree("/repo", Some("refs/heads/main")),
            tree("/repo/feat", Some("feat")),
        ];
        assert_eq!(
            reminder_from_list(&short, "feat").unwrap().path,
            "/repo/feat"
        );

        let full = [
            tree("/repo", Some("refs/heads/main")),
            tree("/repo/feat", Some("refs/heads/feat")),
        ];
        assert_eq!(
            reminder_from_list(&full, "feat").unwrap().path,
            "/repo/feat"
        );
        assert_eq!(
            reminder_from_list(&full, "refs/heads/feat").unwrap().path,
            "/repo/feat"
        );
    }

    #[test]
    fn only_main_match_falls_back_to_first_linked() {
        let trees = [
            tree("/repo", Some("refs/heads/main")),
            tree("/repo/other", Some("refs/heads/other")),
            tree("/repo/third", Some("refs/heads/third")),
        ];
        assert_eq!(
            reminder_from_list(&trees, "main"),
            Some(Reminder {
                branch: "main".into(),
                path: "/repo/other".into(),
            })
        );
    }

    #[test]
    fn detached_linked_worktree_does_not_count_as_branch_match() {
        let trees = [
            tree("/repo", Some("refs/heads/main")),
            tree("/repo/det", None),
        ];
        assert_eq!(reminder_from_list(&trees, "feat"), None);
        assert_eq!(
            reminder_from_list(&trees, "main"),
            Some(Reminder {
                branch: "main".into(),
                path: "/repo/det".into(),
            })
        );
    }
}
