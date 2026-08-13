//! Right-rail diff workbench: porcelain rows plus sort.

/// How the Diff tab orders paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffSort {
    #[default]
    LastTurn,
    FileName,
}

impl DiffSort {
    pub fn label(self) -> &'static str {
        match self {
            Self::LastTurn => "Last turn",
            Self::FileName => "File name",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::LastTurn => Self::FileName,
            Self::FileName => Self::LastTurn,
        }
    }
}

/// One changed path from `git status --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRow {
    pub path: String,
    pub status: String,
    pub last_turn: bool,
}

/// Parse porcelain (`XY path` or `XY old -> new`). Ignores junk lines.
pub fn parse_porcelain(text: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    for line in text.lines() {
        if line.len() < 4 {
            continue;
        }
        let raw: String = line.chars().take(2).collect();
        if !raw
            .chars()
            .all(|c| matches!(c, ' ' | 'M' | 'A' | 'D' | 'R' | 'C' | 'U' | '?' | '!'))
        {
            continue;
        }
        let status = raw.trim().to_owned();
        if status.is_empty() {
            continue;
        }
        let rest = line.chars().skip(3).collect::<String>();
        let rest = rest.trim();
        if rest.is_empty() {
            continue;
        }
        let path = rest
            .rsplit_once(" -> ")
            .map(|(_, dest)| dest)
            .unwrap_or(rest)
            .trim()
            .trim_matches('"')
            .to_owned();
        if path.is_empty() {
            continue;
        }
        rows.push(DiffRow {
            path,
            status,
            last_turn: false,
        });
    }
    rows
}

/// Mark rows whose path is in `last_turn` (exact match).
pub fn mark_last_turn(rows: &mut [DiffRow], last_turn: &[String]) {
    for row in rows {
        row.last_turn = last_turn.iter().any(|p| p == &row.path);
    }
}

/// One-line Changes rail title. OpenChamber / t3Code language.
pub fn changes_headline(rows: &[DiffRow]) -> String {
    match rows.len() {
        0 => "No working-tree changes".into(),
        1 => "1 file changed".into(),
        n => format!("{n} files changed"),
    }
}

/// Compact status letter for a porcelain `XY` pair.
pub fn status_mark(status: &str) -> &'static str {
    if status.contains('A') || status.contains('?') {
        "A"
    } else if status.contains('D') {
        "D"
    } else if status.contains('R') {
        "R"
    } else {
        "M"
    }
}

/// Kind of one line inside a unified hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HunkLineKind {
    Context,
    Add,
    Del,
}

/// One painted line of a unified hunk (includes the leading `+`/`-`/` `).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HunkLine {
    pub kind: HunkLineKind,
    pub text: String,
}

/// One `@@` hunk from `git diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    pub path: String,
    pub header: String,
    pub lines: Vec<HunkLine>,
}

/// Parse unified diffs. Files with no `@@` hunks are skipped.
pub fn parse_unified_diff(text: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut path = String::new();
    let mut current: Option<DiffHunk> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            path = path_from_git_header(rest);
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            if let Some(next) = path_from_plus_line(rest) {
                path = next;
            }
            continue;
        }
        if is_diff_meta(line) {
            continue;
        }
        if line.starts_with("@@") {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            current = Some(DiffHunk {
                path: path.clone(),
                header: line.to_owned(),
                lines: Vec::new(),
            });
            continue;
        }
        if let Some(hunk) = current.as_mut() {
            let kind = if line.starts_with('+') {
                HunkLineKind::Add
            } else if line.starts_with('-') {
                HunkLineKind::Del
            } else {
                HunkLineKind::Context
            };
            hunk.lines.push(HunkLine {
                kind,
                text: line.to_owned(),
            });
        }
    }
    if let Some(hunk) = current.take() {
        hunks.push(hunk);
    }
    hunks
}

fn is_diff_meta(line: &str) -> bool {
    line.starts_with("--- ")
        || line.starts_with("index ")
        || line.starts_with("new file")
        || line.starts_with("deleted file")
        || line.starts_with("similarity ")
        || line.starts_with("rename ")
        || line.starts_with("old mode")
        || line.starts_with("new mode")
}

fn path_from_git_header(rest: &str) -> String {
    rest.split_whitespace()
        .nth(1)
        .or_else(|| rest.split_whitespace().next())
        .unwrap_or("")
        .trim_start_matches("b/")
        .trim_start_matches("a/")
        .to_owned()
}

fn path_from_plus_line(rest: &str) -> Option<String> {
    let token = rest.split('\t').next().unwrap_or(rest).trim();
    if token == "/dev/null" {
        return None;
    }
    Some(
        token
            .trim_start_matches("b/")
            .trim_start_matches("a/")
            .to_owned(),
    )
}

/// Hunks whose path matches `path` exactly.
pub fn hunks_for_path<'a>(hunks: &'a [DiffHunk], path: &str) -> Vec<&'a DiffHunk> {
    hunks.iter().filter(|h| h.path == path).collect()
}

/// Last turn first (then path), or path only.
pub fn sort_diffs(mut rows: Vec<DiffRow>, sort: DiffSort) -> Vec<DiffRow> {
    match sort {
        DiffSort::FileName => {
            rows.sort_by(|a, b| a.path.cmp(&b.path));
        }
        DiffSort::LastTurn => {
            rows.sort_by(|a, b| {
                b.last_turn
                    .cmp(&a.last_turn)
                    .then_with(|| a.path.cmp(&b.path))
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_parses_status_and_rename() {
        let rows = parse_porcelain(
            " M src/lib.rs\n?? new.rs\nR  old.rs -> renamed.rs\n M a\nshort\nxy\n\n",
        );
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[3].path, "a");
        assert_eq!(rows[0].path, "src/lib.rs");
        assert_eq!(rows[0].status, "M");
        assert!(!rows[0].last_turn);
        assert_eq!(rows[1].path, "new.rs");
        assert_eq!(rows[1].status, "??");
        assert_eq!(rows[2].path, "renamed.rs");
        assert_eq!(rows[2].status, "R");
        assert!(parse_porcelain("").is_empty());
        assert!(parse_porcelain("??\n").is_empty());
    }

    #[test]
    fn sort_last_turn_then_name() {
        let mut rows = parse_porcelain(" M zebra.rs\n M alpha.rs\n M mid.rs\n");
        mark_last_turn(&mut rows, &["zebra.rs".into(), "mid.rs".into()]);
        let by_name = sort_diffs(rows.clone(), DiffSort::FileName);
        assert_eq!(
            by_name.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            ["alpha.rs", "mid.rs", "zebra.rs"]
        );
        let by_turn = sort_diffs(rows, DiffSort::LastTurn);
        assert_eq!(
            by_turn.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(),
            ["mid.rs", "zebra.rs", "alpha.rs"]
        );
        assert!(by_turn[0].last_turn && by_turn[1].last_turn);
        assert!(!by_turn[2].last_turn);
        assert_eq!(DiffSort::LastTurn.toggle(), DiffSort::FileName);
        assert_eq!(DiffSort::LastTurn.label(), "Last turn");
        assert_eq!(DiffSort::FileName.label(), "File name");
        assert_eq!(changes_headline(&[]), "No working-tree changes");
        assert_eq!(changes_headline(&by_name[..1]), "1 file changed");
        assert_eq!(changes_headline(&by_name), "3 files changed");
        assert_eq!(status_mark("M"), "M");
        assert_eq!(status_mark("??"), "A");
        assert_eq!(status_mark(" D"), "D");
        assert_eq!(status_mark("R"), "R");
    }

    #[test]
    fn unified_diff_splits_hunks_and_kinds() {
        let text = "\
diff --git a/src/lib.rs b/src/lib.rs
index 111..222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn a() {}
-fn b() {}
+fn c() {}
 fn d() {}
diff --git a/gone.rs b/gone.rs
deleted file mode 100644
--- a/gone.rs
+++ /dev/null
@@ -1 +0,0 @@
-old
";
        let hunks = parse_unified_diff(text);
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].path, "src/lib.rs");
        assert!(hunks[0].header.starts_with("@@"));
        assert_eq!(hunks[0].lines.len(), 4);
        assert_eq!(hunks[0].lines[1].kind, HunkLineKind::Del);
        assert_eq!(hunks[0].lines[2].kind, HunkLineKind::Add);
        assert_eq!(hunks[0].lines[0].kind, HunkLineKind::Context);
        assert_ne!(hunks[0].lines[1].kind, HunkLineKind::Add);
        assert_eq!(hunks[1].path, "gone.rs");
        assert_eq!(hunks[1].lines[0].kind, HunkLineKind::Del);
        assert_eq!(hunks_for_path(&hunks, "src/lib.rs").len(), 1);
        assert!(hunks_for_path(&hunks, "missing.rs").is_empty());
        assert!(parse_unified_diff("").is_empty());
        assert!(parse_unified_diff("loading foo…").is_empty());
        assert!(parse_unified_diff("diff --git a/x b/x\n--- a/x\n+++ b/x\n").is_empty());
        let plus_only = "+++ b/only.rs\n@@ -0,0 +1 @@\n+hi\n";
        let plus = parse_unified_diff(plus_only);
        assert_eq!(plus.len(), 1);
        assert_eq!(plus[0].path, "only.rs");
        assert_eq!(plus[0].lines[0].kind, HunkLineKind::Add);
        assert_eq!(path_from_plus_line("b/only.rs").as_deref(), Some("only.rs"));
        assert_eq!(path_from_plus_line("/dev/null"), None);
        assert_ne!(path_from_plus_line("b/only.rs"), None);
        assert_eq!(path_from_git_header("a/foo.rs b/foo.rs"), "foo.rs");
        assert_eq!(path_from_git_header("foo.rs"), "foo.rs");
        assert_ne!(path_from_git_header("a/foo.rs b/bar.rs"), "foo.rs");
        assert!(is_diff_meta("--- a/x"));
        assert!(is_diff_meta("index 111..222 100644"));
        assert!(is_diff_meta("new file mode 100644"));
        assert!(is_diff_meta("deleted file mode 100644"));
        assert!(is_diff_meta("similarity index 90%"));
        assert!(is_diff_meta("rename from old.rs"));
        assert!(is_diff_meta("old mode 100644"));
        assert!(is_diff_meta("new mode 100755"));
        assert!(!is_diff_meta("+added"));
        assert!(!is_diff_meta("@@ -1 +1 @@"));
        assert!(!is_diff_meta(" context"));
    }
}
