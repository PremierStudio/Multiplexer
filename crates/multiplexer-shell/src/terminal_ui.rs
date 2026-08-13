//! Headless terminal-strip helpers: prompt, history cap, command classification.
//!
//! Desktop renders these strings. No process is spawned here.

/// Hard cap on retained terminal-strip lines. Oldest lines drain first.
pub const TERM_HISTORY_MAX: usize = 80;

/// Prompt shown on the input line of the terminal strip.
pub const TERM_PROMPT: &str = "mux>";

/// Kind of a single terminal-strip line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TermLineKind {
    Input,
    Output,
    Meta,
    Error,
}

/// Prefix a strip line for display.
///
/// `Input` => `"$ {text}"`, `Output` is unchanged, `Meta` => `"· {text}"`,
/// `Error` => `"! {text}"`.
pub fn format_line(kind: TermLineKind, text: &str) -> String {
    match kind {
        TermLineKind::Input => format!("$ {text}"),
        TermLineKind::Output => text.to_owned(),
        TermLineKind::Meta => format!("· {text}"),
        TermLineKind::Error => format!("! {text}"),
    }
}

/// Append `line` and drain from the front so `log` stays at [`TERM_HISTORY_MAX`].
pub fn push_capped(log: &mut Vec<String>, line: String) {
    log.push(line);
    if log.len() > TERM_HISTORY_MAX {
        let drop = log.len() - TERM_HISTORY_MAX;
        log.drain(0..drop);
    }
}

/// Last `max_lines` entries joined with a newline. Empty when `max_lines` is 0.
pub fn visible_tail(log: &[String], max_lines: usize) -> String {
    let start = log.len().saturating_sub(max_lines);
    log[start..].join("\n")
}

/// Commands the desktop should NOT send to cmd.exe (handle in-process).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinCmd {
    Clear,
    Help,
    Cores,
    Mcp,
    Git,
    Checkpoint,
    Skills,
    Unknown,
}

/// Classify a typed line as an in-process builtin, or `None` for the real shell.
///
/// Empty input is not a builtin. A line that starts with `"git "` is a real
/// git invocation and is not intercepted. Exact tokens: `clear`/`cls`,
/// `help`/`?`, `cores`, `mcp`, `git`, `points`/`checkpoint`, `skills`.
pub fn parse_builtin(line: &str) -> Option<BuiltinCmd> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let key = trimmed.to_ascii_lowercase();
    if key.starts_with("git ") {
        return None;
    }
    match key.as_str() {
        "clear" | "cls" => Some(BuiltinCmd::Clear),
        "help" | "?" => Some(BuiltinCmd::Help),
        "cores" => Some(BuiltinCmd::Cores),
        "mcp" => Some(BuiltinCmd::Mcp),
        "git" => Some(BuiltinCmd::Git),
        "points" | "checkpoint" => Some(BuiltinCmd::Checkpoint),
        "skills" => Some(BuiltinCmd::Skills),
        _ => None,
    }
}

/// Static help copy for the `help` / `?` builtin.
pub fn help_text() -> &'static str {
    "Type a shell command and Enter. Builtins: clear, help, cores, mcp, git, points, skills."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_kinds() {
        assert_eq!(TERM_PROMPT, "mux>");
        assert_ne!(TERM_PROMPT, "mux> ");
        assert_eq!(format_line(TermLineKind::Input, "ls"), "$ ls");
        assert_eq!(format_line(TermLineKind::Input, ""), "$ ");
        assert_eq!(format_line(TermLineKind::Output, "ok"), "ok");
        assert_eq!(format_line(TermLineKind::Output, "$ already"), "$ already");
        assert_eq!(format_line(TermLineKind::Meta, "cleared"), "· cleared");
        assert_eq!(format_line(TermLineKind::Error, "nope"), "! nope");
        assert_ne!(format_line(TermLineKind::Output, "ok"), "$ ok");
        assert_ne!(format_line(TermLineKind::Output, "ok"), "! ok");
        assert_ne!(format_line(TermLineKind::Meta, "x"), "! x");
        assert_ne!(format_line(TermLineKind::Error, "x"), "· x");
        assert!(format_line(TermLineKind::Input, "ls").starts_with("$ "));
        assert!(format_line(TermLineKind::Error, "x").starts_with('!'));
        assert!(format_line(TermLineKind::Meta, "x").starts_with('·'));
    }

    #[test]
    fn push_caps() {
        assert_eq!(TERM_HISTORY_MAX, 80);
        let mut log = Vec::new();
        for i in 0..TERM_HISTORY_MAX {
            push_capped(&mut log, format!("l{i}"));
        }
        assert_eq!(log.len(), TERM_HISTORY_MAX);
        assert_eq!(log.first().map(String::as_str), Some("l0"));
        let last_kept = format!("l{}", TERM_HISTORY_MAX - 1);
        assert_eq!(log.last().map(String::as_str), Some(last_kept.as_str()));

        push_capped(&mut log, "overflow".to_owned());
        assert_eq!(log.len(), TERM_HISTORY_MAX);
        assert_eq!(log.first().map(String::as_str), Some("l1"));
        assert_eq!(log.last().map(String::as_str), Some("overflow"));
        assert!(!log.iter().any(|s| s == "l0"));
        assert_eq!(log[TERM_HISTORY_MAX - 1], "overflow");

        assert_eq!(visible_tail(&log, 1), "overflow");
        assert_eq!(
            visible_tail(&["a".into(), "b".into(), "c".into()], 2),
            "b\nc"
        );
        assert_eq!(visible_tail(&["a".into()], 5), "a");
        assert_eq!(visible_tail(&[], 3), "");
        assert_eq!(visible_tail(&["a".into(), "b".into()], 0), "");
        assert!(
            !visible_tail(&["a".into(), "b".into()], 2).contains('\r'),
            "tail is joined with LF only"
        );
        assert_ne!(visible_tail(&["a".into(), "b".into()], 2), "a\nb\n");
    }

    #[test]
    fn parse_builtins() {
        assert_eq!(parse_builtin("clear"), Some(BuiltinCmd::Clear));
        assert_eq!(parse_builtin("cls"), Some(BuiltinCmd::Clear));
        assert_eq!(parse_builtin("help"), Some(BuiltinCmd::Help));
        assert_eq!(parse_builtin("?"), Some(BuiltinCmd::Help));
        assert_eq!(parse_builtin("cores"), Some(BuiltinCmd::Cores));
        assert_eq!(parse_builtin("mcp"), Some(BuiltinCmd::Mcp));
        assert_eq!(parse_builtin("git"), Some(BuiltinCmd::Git));
        assert_eq!(parse_builtin("points"), Some(BuiltinCmd::Checkpoint));
        assert_eq!(parse_builtin("checkpoint"), Some(BuiltinCmd::Checkpoint));
        assert_eq!(parse_builtin("skills"), Some(BuiltinCmd::Skills));
        assert_eq!(parse_builtin("  clear  "), Some(BuiltinCmd::Clear));
        assert_eq!(parse_builtin("CLEAR"), Some(BuiltinCmd::Clear));
        assert_eq!(parse_builtin("ls"), None);
        assert_eq!(parse_builtin("clear now"), None);
        assert_eq!(parse_builtin("cores --all"), None);
        assert_eq!(parse_builtin("mcp list"), None);
        assert_ne!(parse_builtin("help"), Some(BuiltinCmd::Clear));
        assert_ne!(parse_builtin("cores"), Some(BuiltinCmd::Mcp));
        assert!(help_text().contains("git"));
        assert!(help_text().contains("points"));
        assert!(help_text().contains("clear, help, cores, mcp, git, points"));
        assert_eq!(
            help_text(),
            "Type a shell command and Enter. Builtins: clear, help, cores, mcp, git, points, skills."
        );
    }

    #[test]
    fn empty_not_builtin() {
        assert_eq!(parse_builtin(""), None);
        assert_eq!(parse_builtin("   "), None);
        assert_eq!(parse_builtin("\t\n"), None);
        assert_ne!(parse_builtin(""), Some(BuiltinCmd::Unknown));
        assert_ne!(parse_builtin(""), Some(BuiltinCmd::Help));
    }

    #[test]
    fn git_status_not_builtin() {
        assert_eq!(parse_builtin("git status"), None);
        assert_eq!(parse_builtin("git  status"), None);
        assert_eq!(parse_builtin("GIT STATUS"), None);
        assert_eq!(parse_builtin("git diff --stat"), None);
        assert_eq!(parse_builtin("  git status"), None);
        assert_ne!(parse_builtin("git status"), Some(BuiltinCmd::Git));
        assert_eq!(parse_builtin("git"), Some(BuiltinCmd::Git));
    }
}
