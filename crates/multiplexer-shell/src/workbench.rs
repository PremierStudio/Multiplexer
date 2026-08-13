//! Workbench helpers: files filter, MCP merge, browser detect, activity.

use std::path::{Path, PathBuf};

use crate::workspace::McpRow;
use crate::Workspace;

/// Case-insensitive name filter. Empty query returns all.
pub fn filter_files(files: &[String], query: &str) -> Vec<String> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return files.to_vec();
    }
    files
        .iter()
        .filter(|f| f.to_ascii_lowercase().contains(&q))
        .cloned()
        .collect()
}

/// Join project root and a relative leaf. `..` segments are kept (caller copies).
pub fn join_project_path(project: &str, rel: &str) -> PathBuf {
    let rel = rel.trim_end_matches('/');
    Path::new(project).join(rel)
}

/// Incoming inventory wins on command/transport. Ready/Stopped follows the
/// existing row of the same name so Reload does not reset a flag.
pub fn merge_mcp(existing: &[McpRow], incoming: Vec<McpRow>) -> Vec<McpRow> {
    incoming
        .into_iter()
        .map(|mut row| {
            if let Some(old) = existing.iter().find(|e| e.name == row.name) {
                row.state = old.state;
            }
            row
        })
        .collect()
}

/// Well-known Windows browser binaries. Detect is path-exists only.
pub fn default_browser_candidates() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "edge",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        ),
        (
            "chrome",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        ),
        ("firefox", r"C:\Program Files\Mozilla Firefox\firefox.exe"),
        (
            "brave",
            r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        ),
    ]
}

pub fn detect_browsers(candidates: &[(&str, &str)]) -> Vec<(String, String)> {
    candidates
        .iter()
        .filter(|(_, path)| Path::new(path).is_file())
        .map(|(name, path)| ((*name).to_owned(), (*path).to_owned()))
        .collect()
}

/// First detected browser, else none. UI still falls back to `start`.
pub fn preferred_browser(found: &[(String, String)]) -> Option<&(String, String)> {
    found.first()
}

/// One installed system terminal. `id` is stable (`wt`, `cmd`, `conhost`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemTerminal {
    pub id: String,
    pub label: String,
    pub path: String,
}

/// Well-known **shells** we can host in an in-app PTY (not GUI terminal apps).
pub fn default_terminal_candidates() -> Vec<(String, String, String)> {
    #[cfg(windows)]
    {
        windows_terminal_candidates()
    }
    #[cfg(target_os = "macos")]
    {
        macos_terminal_candidates()
    }
    #[cfg(target_os = "linux")]
    {
        linux_terminal_candidates()
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

pub fn windows_terminal_candidates() -> Vec<(String, String, String)> {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let mut out = vec![
        (
            "cmd".into(),
            "Command Prompt".into(),
            r"C:\Windows\System32\cmd.exe".into(),
        ),
        (
            "powershell".into(),
            "Windows PowerShell".into(),
            r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe".into(),
        ),
        (
            "pwsh".into(),
            "PowerShell".into(),
            format!("{local}\\Microsoft\\WindowsApps\\pwsh.exe"),
        ),
    ];
    if let Some(grok) = path_program("grok") {
        out.insert(0, ("grok".into(), "Grok".into(), grok));
    }
    out
}

pub fn macos_terminal_candidates() -> Vec<(String, String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out = vec![
        ("zsh".into(), "zsh".into(), "/bin/zsh".into()),
        ("bash".into(), "bash".into(), "/bin/bash".into()),
        ("sh".into(), "sh".into(), "/bin/sh".into()),
        (
            "fish".into(),
            "fish".into(),
            "/opt/homebrew/bin/fish".into(),
        ),
        (
            "fish".into(),
            "fish".into(),
            format!("{home}/.local/bin/fish"),
        ),
    ];
    if let Some(grok) = path_program("grok") {
        out.insert(0, ("grok".into(), "Grok".into(), grok));
    }
    out
}

pub fn linux_terminal_candidates() -> Vec<(String, String, String)> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out = vec![
        ("bash".into(), "bash".into(), "/bin/bash".into()),
        ("zsh".into(), "zsh".into(), "/bin/zsh".into()),
        ("sh".into(), "sh".into(), "/bin/sh".into()),
        ("fish".into(), "fish".into(), "/usr/bin/fish".into()),
        (
            "fish".into(),
            "fish".into(),
            format!("{home}/.local/bin/fish"),
        ),
    ];
    if let Some(grok) = path_program("grok") {
        out.insert(0, ("grok".into(), "Grok".into(), grok));
    }
    out
}

/// First executable named `name` on PATH (adds `.exe` on Windows).
pub fn path_program(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct.display().to_string());
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe.display().to_string());
            }
        }
    }
    None
}

pub fn detect_terminals(candidates: &[(String, String, String)]) -> Vec<SystemTerminal> {
    candidates
        .iter()
        .filter(|(_, _, path)| std::path::Path::new(path).exists())
        .map(|(id, label, path)| SystemTerminal {
            id: id.clone(),
            label: label.clone(),
            path: path.clone(),
        })
        .collect()
}

pub fn preferred_terminal<'a>(
    found: &'a [SystemTerminal],
    want: &str,
) -> Option<&'a SystemTerminal> {
    if !want.trim().is_empty() {
        if let Some(hit) = found.iter().find(|t| t.id == want) {
            return Some(hit);
        }
    }
    found.first()
}

/// Stable activity rows for left and right Activity. Cap 20.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityItem {
    pub id: String,
    pub title: String,
    pub hint: String,
}

pub fn activity_items(ws: &Workspace) -> Vec<ActivityItem> {
    let mut items = Vec::new();
    if let Some((branch, path)) = &ws.reminder {
        items.push(ActivityItem {
            id: "act:reminder".into(),
            title: format!("worktree {branch}"),
            hint: path.clone(),
        });
    }
    if let Some(p) = &ws.pending {
        items.push(ActivityItem {
            id: format!("act:approval:{}", p.request_id),
            title: p.card_title(),
            hint: p.card_body(),
        });
    }
    if ws.busy {
        items.push(ActivityItem {
            id: "act:busy".into(),
            title: "Grok is working".into(),
            hint: "headless grok -p".into(),
        });
    }
    for n in ws.notices.iter().rev().take(5) {
        items.push(ActivityItem {
            id: format!("act:notice:{}", n.id),
            title: n.text.clone(),
            hint: format!("{:?}", n.kind),
        });
    }
    for (i, line) in ws.terminal_log.iter().rev().take(8).enumerate() {
        items.push(ActivityItem {
            id: format!("act:log:{i}"),
            title: line.chars().take(72).collect(),
            hint: "term".into(),
        });
    }
    if items.is_empty() {
        items.push(ActivityItem {
            id: "act:empty".into(),
            title: "No activity yet".into(),
            hint: "open Activity".into(),
        });
    }
    items.truncate(20);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::McpLife;

    #[test]
    fn filter_files_empty_and_narrow() {
        let files = vec![
            "src/main.rs".into(),
            "Cargo.toml".into(),
            "src/lib.rs".into(),
        ];
        assert_eq!(filter_files(&files, "").len(), 3);
        assert_eq!(filter_files(&files, "  ").len(), 3);
        let rs = filter_files(&files, "LIB");
        assert_eq!(rs, vec!["src/lib.rs".to_owned()]);
        assert!(filter_files(&files, "zzz").is_empty());
        assert_ne!(filter_files(&files, "src"), filter_files(&files, "toml"));
    }

    #[test]
    fn join_project_keeps_leaf() {
        let p = join_project_path("C:/repo", "src/main.rs");
        assert!(p.ends_with("main.rs"));
        let dir = join_project_path("C:/repo", "src/");
        assert!(dir.ends_with("src"));
    }

    #[test]
    fn merge_mcp_keeps_ready_by_name() {
        let existing = vec![McpRow {
            name: "linear".into(),
            command: "old".into(),
            transport: "stdio".into(),
            state: McpLife::Ready,
        }];
        let incoming = vec![
            McpRow {
                name: "linear".into(),
                command: "npx".into(),
                transport: "stdio".into(),
                state: McpLife::Stopped,
            },
            McpRow {
                name: "gh".into(),
                command: "gh".into(),
                transport: "stdio".into(),
                state: McpLife::Stopped,
            },
        ];
        let merged = merge_mcp(&existing, incoming);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].command, "npx");
        assert_eq!(merged[0].state, McpLife::Ready);
        assert_eq!(merged[1].state, McpLife::Stopped);
        let fresh = merge_mcp(&[], vec![existing[0].clone()]);
        assert_eq!(fresh[0].state, McpLife::Ready);
    }

    #[test]
    fn detect_browsers_only_existing_files() {
        let dir = std::env::temp_dir().join(format!("mux-brow-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let exe = dir.join("msedge.exe");
        std::fs::write(&exe, b"x").unwrap();
        let missing = dir.join("nope.exe");
        let found = detect_browsers(&[
            ("edge", exe.to_str().unwrap()),
            ("chrome", missing.to_str().unwrap()),
        ]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "edge");
        assert_eq!(
            preferred_browser(&found).map(|b| b.0.as_str()),
            Some("edge")
        );
        assert!(preferred_browser(&[]).is_none());
        assert!(!default_browser_candidates().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detect_terminals_only_existing_files() {
        let dir = std::env::temp_dir().join(format!("mux-term-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let wt = dir.join("wt.exe");
        let cmd = dir.join("cmd.exe");
        std::fs::write(&wt, b"x").unwrap();
        std::fs::write(&cmd, b"x").unwrap();
        let missing = dir.join("nope.exe");
        let found = detect_terminals(&[
            (
                "wt".into(),
                "Windows Terminal".into(),
                wt.to_string_lossy().into(),
            ),
            (
                "cmd".into(),
                "Command Prompt".into(),
                cmd.to_string_lossy().into(),
            ),
            (
                "missing".into(),
                "Missing".into(),
                missing.to_string_lossy().into(),
            ),
        ]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].id, "wt");
        assert_eq!(found[1].id, "cmd");
        assert_ne!(found[0].id, "cmd");
        assert_eq!(
            preferred_terminal(&found, "").map(|t| t.id.as_str()),
            Some("wt")
        );
        assert_eq!(
            preferred_terminal(&found, "   ").map(|t| t.id.as_str()),
            Some("wt")
        );
        assert_eq!(
            preferred_terminal(&found, "wt").map(|t| t.label.as_str()),
            Some("Windows Terminal")
        );
        assert_eq!(
            preferred_terminal(&found, "cmd").map(|t| t.id.as_str()),
            Some("cmd")
        );
        assert_ne!(
            preferred_terminal(&found, "cmd").map(|t| t.id.as_str()),
            preferred_terminal(&found, "").map(|t| t.id.as_str())
        );
        assert_eq!(
            preferred_terminal(&found, "missing").map(|t| t.id.as_str()),
            Some("wt"),
            "unknown want falls back to first"
        );
        assert!(preferred_terminal(&[], "wt").is_none());
        assert!(preferred_terminal(&[], "").is_none());
        assert!(windows_terminal_candidates()
            .iter()
            .any(|c| c.0 == "cmd" && c.1 == "Command Prompt"));
        assert!(windows_terminal_candidates()
            .iter()
            .any(|c| c.0 == "powershell"));
        assert!(!windows_terminal_candidates().iter().any(|c| c.0 == "wt"));
        assert!(macos_terminal_candidates().iter().any(|c| c.0 == "zsh"));
        assert!(macos_terminal_candidates().iter().any(|c| c.1 == "zsh"));
        assert!(!macos_terminal_candidates().iter().any(|c| c.0 == "iterm"));
        assert!(linux_terminal_candidates().iter().any(|c| c.0 == "bash"));
        assert!(!linux_terminal_candidates()
            .iter()
            .any(|c| c.0 == "gnome-terminal"));
        assert!(default_terminal_candidates()
            .iter()
            .any(|c| c.0 == "cmd" || c.0 == "zsh" || c.0 == "bash"));
        assert!(path_program("mux-no-such-binary-xyz").is_none());
        let cargo = path_program("cargo");
        assert!(
            cargo
                .as_ref()
                .is_some_and(|p| p.to_ascii_lowercase().contains("cargo")),
            "cargo must be on PATH, got {cargo:?}"
        );
        assert_ne!(cargo, Some(String::new()));
        assert_ne!(cargo, Some("xyzzy".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn activity_includes_reminder_notice_and_empty() {
        let mut ws = Workspace::new("p", "m");
        let empty = activity_items(&ws);
        assert_eq!(empty.len(), 1);
        assert_eq!(empty[0].id, "act:empty");
        ws.set_reminder("main", "C:/repo");
        ws.push_notice(crate::NoticeKind::Info, "hi");
        ws.busy = true;
        ws.terminal_log.push("$ git status".into());
        let items = activity_items(&ws);
        assert!(items.iter().any(|i| i.id == "act:reminder"));
        assert!(items.iter().any(|i| i.id == "act:busy"));
        assert!(items.iter().any(|i| i.id.starts_with("act:notice:")));
        assert!(items.iter().any(|i| i.id.starts_with("act:log:")));
        assert!(items.iter().all(|i| i.id != "act:empty"));
        assert!(items.len() <= 20);
    }
}
