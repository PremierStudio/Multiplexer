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
}
