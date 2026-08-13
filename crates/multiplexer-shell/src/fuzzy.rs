//! Subsequence fuzzy score for the command palette.

/// Score a case-insensitive subsequence match.
///
/// `None` when `query` is not a subsequence of `text`. An empty query scores 0.
/// Prefix, word-start, and consecutive runs score higher than gapped matches.
pub fn fuzzy_score(query: &str, text: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    let q: Vec<char> = query.to_ascii_lowercase().chars().collect();
    let t: Vec<char> = text.to_ascii_lowercase().chars().collect();
    if q.len() > t.len() {
        return None;
    }
    let mut qi = 0usize;
    let mut score = 0u32;
    let mut run = 0u32;
    let mut first = None;
    for (i, &ch) in t.iter().enumerate() {
        if qi < q.len() && ch == q[qi] {
            if first.is_none() {
                first = Some(i);
            }
            run = run.saturating_add(1);
            score = score.saturating_add(8);
            score = score.saturating_add(run.saturating_mul(6));
            if i == 0 || is_word_break(t[i - 1]) {
                score = score.saturating_add(24);
            }
            qi += 1;
        } else {
            run = 0;
        }
    }
    if qi != q.len() {
        return None;
    }
    if let Some(at) = first {
        score = score.saturating_add(800u32.saturating_sub((at as u32).saturating_mul(3)));
    }
    let lower = text.to_ascii_lowercase();
    let qlow = query.to_ascii_lowercase();
    if lower.starts_with(&qlow) {
        score = score.saturating_add(400);
    }
    if lower == qlow {
        score = score.saturating_add(200);
    }
    Some(score)
}

fn is_word_break(ch: char) -> bool {
    matches!(ch, ' ' | '-' | '_' | '/' | '\\' | '.' | ':' | '+')
}

/// Best score across several haystacks. `None` when none match.
pub fn fuzzy_best(query: &str, parts: &[&str]) -> Option<u32> {
    let mut best = None;
    for part in parts {
        if let Some(score) = fuzzy_score(query, part) {
            best = Some(best.map_or(score, |b: u32| b.max(score)));
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_scores_zero() {
        assert_eq!(fuzzy_score("", "Create checkpoint"), Some(0));
        assert_eq!(fuzzy_score("", ""), Some(0));
        assert_ne!(fuzzy_score("", "x"), None);
    }

    #[test]
    fn subsequence_required() {
        assert!(fuzzy_score("cp", "Create checkpoint").is_some());
        assert!(fuzzy_score("CP", "create checkpoint").is_some());
        assert_eq!(fuzzy_score("zzz", "Create checkpoint"), None);
        assert_eq!(fuzzy_score("xyz", "abc"), None);
        assert_eq!(fuzzy_score("abcd", "abc"), None);
    }

    #[test]
    fn prefix_beats_later_match() {
        let prefix = fuzzy_score("git", "git status").unwrap();
        let later = fuzzy_score("git", "refresh git").unwrap();
        assert!(prefix > later, "{prefix} vs {later}");
        let exact = fuzzy_score("git", "git").unwrap();
        assert!(exact > prefix, "{exact} vs {prefix}");
    }

    #[test]
    fn consecutive_beats_gapped() {
        let tight = fuzzy_score("cp", "cp").unwrap();
        let gapped = fuzzy_score("cp", "create point").unwrap();
        assert!(tight > gapped, "{tight} vs {gapped}");
    }

    #[test]
    fn word_start_bonus() {
        let word = fuzzy_score("cp", "Create checkpoint").unwrap();
        let mid = fuzzy_score("cp", "scope").unwrap();
        assert!(word > mid, "{word} vs {mid}");
    }

    #[test]
    fn mcp_does_not_hit_palette() {
        assert!(fuzzy_score("mcp", "mcp").is_some());
        assert!(fuzzy_score("mcp", "Refresh MCP").is_some());
        assert_eq!(fuzzy_score("mcp", "Command palette"), None);
        assert_eq!(fuzzy_score("mcp", "Toggle chats"), None);
    }

    #[test]
    fn fuzzy_best_picks_highest() {
        assert_eq!(fuzzy_best("zz", &["one", "two"]), None);
        let score = fuzzy_best("git", &["refresh git", "git"]).unwrap();
        assert_eq!(score, fuzzy_score("git", "git").unwrap());
        assert!(score > fuzzy_score("git", "refresh git").unwrap());
    }

    #[test]
    fn word_break_chars() {
        assert!(is_word_break(' '));
        assert!(is_word_break('-'));
        assert!(is_word_break('_'));
        assert!(is_word_break('/'));
        assert!(!is_word_break('a'));
        assert!(!is_word_break('1'));
        assert!(fuzzy_score("rs", "src/main.rs").is_some());
        let after_hyphen = fuzzy_score("cp", "xx-cp").unwrap();
        let mid = fuzzy_score("cp", "xcxp").unwrap();
        assert!(after_hyphen > mid, "{after_hyphen} vs {mid}");
    }
}
