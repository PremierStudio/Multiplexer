//! Review-workbench helpers: handshake, git header, cores merge, chat copy.

use crate::workspace::CoreRow;
use crate::ConnectionState;

/// Banner under the chat header. Honest about grok -p.
pub fn context_strip() -> &'static str {
    "Grok does not see earlier bubbles. This send is a new process."
}

/// Working-row copy while a headless turn is in flight.
pub fn working_copy(elapsed_secs: u64) -> String {
    let body = if elapsed_secs >= 60 {
        format!("{}m {}s", elapsed_secs / 60, elapsed_secs % 60)
    } else {
        format!("{elapsed_secs}s")
    };
    format!("Grok is working · {body}")
}

/// Ready only after both hello and ping succeed.
pub fn handshake_state(hello_ok: bool, ping_ok: bool) -> ConnectionState {
    if hello_ok && ping_ok {
        ConnectionState::Ready
    } else if hello_ok || ping_ok {
        ConnectionState::Connecting
    } else {
        ConnectionState::Disconnected
    }
}

/// `## main...origin/main [ahead 2, behind 1]`
pub fn git_ahead_behind(status: &str) -> Option<(i64, i64)> {
    let line = status.lines().next().unwrap_or("");
    let ahead = capture_count(line, "ahead ");
    let behind = capture_count(line, "behind ");
    if ahead == 0 && behind == 0 {
        None
    } else {
        Some((ahead, behind))
    }
}

fn capture_count(line: &str, key: &str) -> i64 {
    let Some(rest) = line.split(key).nth(1) else {
        return 0;
    };
    rest.chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Dirty when any porcelain line is not the `##` header.
pub fn git_dirty(status: &str) -> bool {
    status.lines().any(|l| {
        let t = l.trim();
        !t.is_empty() && !t.starts_with("##")
    })
}

pub fn git_header(project: &str, branch: &str, status: &str) -> String {
    let dirty = if git_dirty(status) { "dirty" } else { "clean" };
    let ab = match git_ahead_behind(status) {
        Some((a, b)) => format!("ahead {a} behind {b}"),
        None => "even".to_owned(),
    };
    format!("{project}\n{branch}\n{dirty}\n{ab}")
}

/// Incoming samples win usage. Reserved flag is kept by index.
pub fn merge_cores(existing: &[CoreRow], incoming: Vec<CoreRow>) -> Vec<CoreRow> {
    incoming
        .into_iter()
        .map(|mut row| {
            if let Some(old) = existing.iter().find(|e| e.index == row.index) {
                row.reserved = old.reserved;
            }
            row
        })
        .collect()
}

pub const DIFF_TEXT_CAP: usize = 64 * 1024;

/// Cap preview text. Empty stays empty.
pub fn cap_text(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Unstaged first. Caller may run cached next.
pub fn git_diff_line(path: &str) -> String {
    format!("git diff -- {path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_copy_formats_seconds_and_minutes() {
        assert_eq!(working_copy(0), "Grok is working · 0s");
        assert_eq!(working_copy(9), "Grok is working · 9s");
        assert_eq!(working_copy(60), "Grok is working · 1m 0s");
        assert_eq!(working_copy(75), "Grok is working · 1m 15s");
        assert_ne!(working_copy(5), working_copy(6));
        assert!(context_strip().contains("new process"));
        assert!(!context_strip().contains("streaming"));
    }

    #[test]
    fn handshake_needs_both() {
        assert_eq!(handshake_state(true, true), ConnectionState::Ready);
        assert_eq!(handshake_state(true, false), ConnectionState::Connecting);
        assert_eq!(handshake_state(false, true), ConnectionState::Connecting);
        assert_eq!(handshake_state(false, false), ConnectionState::Disconnected);
        assert_ne!(handshake_state(true, false), handshake_state(true, true));
    }

    #[test]
    fn ahead_behind_and_dirty() {
        let st = "## main...origin/main [ahead 2, behind 1]\n M src/lib.rs\n";
        assert_eq!(git_ahead_behind(st), Some((2, 1)));
        assert_eq!(
            git_ahead_behind("## main...origin/main [ahead 3]"),
            Some((3, 0))
        );
        assert_eq!(
            git_ahead_behind("## main...origin/main [behind 4]"),
            Some((0, 4))
        );
        assert!(git_dirty(st));
        assert_eq!(git_ahead_behind("## main"), None);
        assert!(!git_dirty("## main\n"));
        assert!(!git_dirty(""));
        let header = git_header("C:/repo", "feat", st);
        assert!(header.contains("C:/repo"));
        assert!(header.contains("feat"));
        assert!(header.contains("dirty"));
        assert!(header.contains("ahead 2 behind 1"));
        let clean = git_header("C:/repo", "main", "## main");
        assert!(clean.contains("clean"));
        assert!(clean.contains("even"));
    }

    #[test]
    fn merge_cores_keeps_reserved_flag() {
        let existing = vec![
            CoreRow {
                index: 0,
                usage: 1.0,
                reserved: true,
            },
            CoreRow {
                index: 1,
                usage: 2.0,
                reserved: false,
            },
        ];
        let incoming = vec![
            CoreRow {
                index: 0,
                usage: 40.0,
                reserved: false,
            },
            CoreRow {
                index: 1,
                usage: 10.0,
                reserved: true,
            },
            CoreRow {
                index: 2,
                usage: 3.0,
                reserved: false,
            },
        ];
        let merged = merge_cores(&existing, incoming);
        assert_eq!(merged.len(), 3);
        assert!(merged[0].reserved);
        assert_eq!(merged[0].usage, 40.0);
        assert!(!merged[1].reserved);
        assert!(!merged[2].reserved);
    }

    #[test]
    fn cap_text_and_diff_line() {
        assert_eq!(cap_text("abc", 8), "abc");
        assert_eq!(cap_text("abcdef", 3), "abc…");
        let clipped = cap_text("ééé", 2);
        assert!(clipped.ends_with('…'));
        assert!(clipped.starts_with('é'));
        assert_ne!(clipped, "ééé");
        assert!(cap_text("", 10).is_empty());
        assert!(cap_text("abc", 0).is_empty());
        assert_eq!(git_diff_line("src/a.rs"), "git diff -- src/a.rs");
        assert_ne!(git_diff_line("a"), git_diff_line("b"));
        assert_eq!(DIFF_TEXT_CAP, 64 * 1024);
    }
}
