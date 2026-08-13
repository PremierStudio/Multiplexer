//! Inspector tabs as expandable list rows, not text dumps.

use crate::icons::{BrandIcon, ChromeGlyph};
use crate::widgets::{BadgeSpec, ListRowSpec, Tone};
use crate::workspace::{InspectorTab, Workspace};

/// Rows for the active inspector tab.
pub fn inspector_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    match ws.inspector {
        InspectorTab::Session => session_rows(ws),
        InspectorTab::Resources => core_rows(ws),
        InspectorTab::Mcp => mcp_rows(ws),
        InspectorTab::Checkpoints => checkpoint_rows(ws),
        InspectorTab::Git => git_rows(ws),
        InspectorTab::Terminal => term_rows(ws),
        InspectorTab::Skills => skill_rows(ws),
        InspectorTab::Files => file_rows(ws),
        InspectorTab::Activity => activity_rows(ws),
        InspectorTab::Agents => agent_rows(ws),
        InspectorTab::Diff => diff_rows(ws),
        InspectorTab::Browser => browser_rows(ws),
    }
}

/// Expanded copy for one inspector row id.
pub fn row_detail(ws: &Workspace, id: &str) -> String {
    if id == "session:project" {
        return ws.project.clone();
    }
    if id == "session:model" {
        return ws.model.clone();
    }
    if id == "session:connection" {
        return ws.connection.status_label().to_owned();
    }
    if id == "session:id" {
        return match &ws.connection {
            crate::ConnectionState::Connected { session_ids } => session_ids
                .first()
                .cloned()
                .unwrap_or_else(|| "(none)".into()),
            _ => "(none yet)".into(),
        };
    }
    if id == "session:threads" {
        return ws.threads.len().to_string();
    }
    if id == "session:turns" {
        return ws.usage_lines();
    }
    if id == "git:status" {
        return ws.git_status.clone();
    }
    if let Some(name) = id.strip_prefix("mcp:") {
        return ws
            .mcp
            .iter()
            .find(|m| m.name == name)
            .map(|m| {
                format!(
                    "{} {} inventory flag (no child)",
                    m.command,
                    m.state.label()
                )
            })
            .unwrap_or_default();
    }
    if let Some(path) = id.strip_prefix("file:") {
        let selected = ws.selected_file.as_deref() == Some(path);
        return if selected {
            format!("{path} selected")
        } else {
            format!("{path} not selected")
        };
    }
    if let Some(cid) = id.strip_prefix("point:") {
        return ws
            .checkpoints
            .iter()
            .find(|c| c.id == cid)
            .map(|c| c.label.clone())
            .unwrap_or_default();
    }
    if let Some(path) = id.strip_prefix("diff:") {
        return ws
            .diff_rows
            .iter()
            .find(|r| r.path == path)
            .map(|r| {
                format!(
                    "{}  {}  {}",
                    r.status,
                    r.path,
                    if r.last_turn { "last turn" } else { "earlier" }
                )
            })
            .unwrap_or_default();
    }
    if id == "browser:slot" {
        return ws.browser_detail();
    }
    if let Some(aid) = id.strip_prefix("agent:") {
        return ws
            .threads
            .iter()
            .find(|t| t.id == aid)
            .map(|t| format!("{} {}", t.title, t.status))
            .unwrap_or_default();
    }
    String::new()
}

fn mark_expanded(mut row: ListRowSpec, ws: &Workspace) -> ListRowSpec {
    row.expanded = ws.right_expanded_id.as_deref() == Some(row.id.as_str());
    row
}

fn session_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    let sid = match &ws.connection {
        crate::ConnectionState::Connected { session_ids } => session_ids
            .first()
            .cloned()
            .unwrap_or_else(|| "(none)".into()),
        _ => "(none yet)".into(),
    };
    [
        ListRowSpec::new("session:project", "Project")
            .with_icon(ChromeGlyph::Folder.mark())
            .with_subtitle(&ws.project),
        ListRowSpec::new("session:model", "Model")
            .with_icon(ChromeGlyph::Sparkle.mark())
            .with_subtitle(&ws.model)
            .with_badge(BadgeSpec::new(Tone::Accent, ws.model.clone())),
        ListRowSpec::new("session:connection", "Connection")
            .with_icon(ChromeGlyph::Activity.mark())
            .with_subtitle(ws.connection.status_label()),
        ListRowSpec::new("session:id", "Session")
            .with_icon(ChromeGlyph::Agent.mark())
            .with_subtitle(sid),
        ListRowSpec::new("session:threads", "Threads")
            .with_icon(ChromeGlyph::Chat.mark())
            .with_meta(ws.threads.len().to_string()),
        ListRowSpec::new("session:turns", "Turns")
            .with_icon(ChromeGlyph::Activity.mark())
            .with_subtitle(format!("{} local", ws.usage_turns))
            .with_meta(format!("{} tok", ws.usage_tokens)),
    ]
    .into_iter()
    .map(|r| mark_expanded(r, ws))
    .collect()
}

fn core_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.cores.is_empty() {
        return vec![
            ListRowSpec::new("core:empty", "No core samples").with_icon(ChromeGlyph::Cpu.mark())
        ];
    }
    ws.cores
        .iter()
        .map(|c| {
            let badge = if c.reserved {
                Some(BadgeSpec::new(Tone::Accent, "reserved"))
            } else {
                None
            };
            let mut row = ListRowSpec::new(format!("core:{}", c.index), format!("cpu{}", c.index))
                .with_icon(ChromeGlyph::Cpu.mark())
                .with_subtitle(format!("{:.1}%", c.usage))
                .with_meta(crate::usage_bar(c.usage, 10));
            row.badge = badge;
            mark_expanded(row, ws)
        })
        .collect()
}

fn mcp_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    const SUPERVISED: &str = "inventory flag (no child)";
    if ws.mcp.is_empty() {
        return vec![ListRowSpec::new("mcp:empty", "No MCP servers")
            .with_icon(ChromeGlyph::Plug.mark())
            .with_subtitle(SUPERVISED)];
    }
    ws.mcp
        .iter()
        .map(|m| {
            let icon = BrandIcon::from_name(&m.name)
                .or_else(|| BrandIcon::from_name(&m.command))
                .map(|b| b.slug().to_string())
                .unwrap_or_else(|| ChromeGlyph::Plug.mark().to_string());
            let tone = match m.state {
                crate::workspace::McpLife::Ready => Tone::Neutral,
                crate::workspace::McpLife::Crashed | crate::workspace::McpLife::Failed => {
                    Tone::Danger
                }
                crate::workspace::McpLife::Stopped => Tone::Neutral,
            };
            mark_expanded(
                ListRowSpec::new(format!("mcp:{}", m.name), m.name.clone())
                    .with_icon(icon)
                    .with_subtitle(format!("{} · {SUPERVISED}", m.command))
                    .with_badge(BadgeSpec::new(tone, m.state.label())),
                ws,
            )
        })
        .collect()
}

fn checkpoint_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.checkpoints.is_empty() {
        return vec![
            ListRowSpec::new("point:empty", "No checkpoints").with_icon(ChromeGlyph::Flag.mark())
        ];
    }
    ws.checkpoints
        .iter()
        .map(|c| {
            let mut row = ListRowSpec::new(format!("point:{}", c.id), c.label.clone())
                .with_icon(ChromeGlyph::Flag.mark())
                .with_subtitle(c.id.clone());
            row.selected = ws.selected_checkpoint.as_deref() == Some(c.id.as_str());
            mark_expanded(row, ws)
        })
        .collect()
}

fn git_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    let mut rows: Vec<ListRowSpec> = ws
        .worktrees
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let mut row = ListRowSpec::new(format!("git:wt:{i}"), path.clone())
                .with_icon(ChromeGlyph::Git.mark());
            row.selected = ws.selected_worktree == Some(i);
            mark_expanded(row, ws)
        })
        .collect();
    if !ws.git_status.is_empty() {
        rows.push(mark_expanded(
            ListRowSpec::new("git:status", "git status")
                .with_icon(ChromeGlyph::Diff.mark())
                .with_subtitle(ws.git_status.lines().next().unwrap_or("").to_owned()),
            ws,
        ));
    }
    if rows.is_empty() {
        rows.push(ListRowSpec::new("git:empty", "No worktrees").with_icon(ChromeGlyph::Git.mark()));
    }
    rows
}

fn term_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    let mut rows: Vec<ListRowSpec> = ws
        .terminal_log
        .iter()
        .enumerate()
        .rev()
        .take(12)
        .map(|(i, line)| {
            mark_expanded(
                ListRowSpec::new(format!("term:{i}"), line.clone())
                    .with_icon(ChromeGlyph::Terminal.mark()),
                ws,
            )
        })
        .collect();
    rows.insert(
        0,
        mark_expanded(
            ListRowSpec::new("term:draft", "Draft")
                .with_icon(ChromeGlyph::Terminal.mark())
                .with_subtitle(if ws.term_draft.is_empty() {
                    "(empty)".into()
                } else {
                    ws.term_draft.clone()
                }),
            ws,
        ),
    );
    rows
}

fn skill_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.skill_items.is_empty() && ws.skills.is_empty() {
        return vec![ListRowSpec::new("skill:empty", "No skills")
            .with_icon(ChromeGlyph::Sparkle.mark())
            .with_subtitle(".grok/skills")];
    }
    if !ws.skill_items.is_empty() {
        return ws
            .skill_items
            .iter()
            .map(|s| {
                let flag = if s.enabled { "on" } else { "off" };
                mark_expanded(
                    ListRowSpec::new(format!("skill:{}", s.name), s.name.clone())
                        .with_icon(ChromeGlyph::Sparkle.mark())
                        .with_subtitle(format!("{} · {flag} · not loaded into grok", s.source))
                        .with_badge(BadgeSpec::new(
                            if s.enabled { Tone::Good } else { Tone::Neutral },
                            flag,
                        )),
                    ws,
                )
            })
            .collect();
    }
    ws.skills
        .iter()
        .map(|s| {
            mark_expanded(
                ListRowSpec::new(format!("skill:{s}"), s.clone())
                    .with_icon(ChromeGlyph::Sparkle.mark()),
                ws,
            )
        })
        .collect()
}

fn file_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.files.is_empty() {
        return vec![
            ListRowSpec::new("file:empty", "No files").with_icon(ChromeGlyph::Folder.mark())
        ];
    }
    ws.files_visible()
        .into_iter()
        .map(|p| {
            let icon = if p.ends_with('/') {
                ChromeGlyph::Folder.mark()
            } else {
                ChromeGlyph::Diff.mark()
            };
            let selected = ws.selected_file.as_deref() == Some(p.as_str());
            let leaf = crate::persist::leaf_name(&p);
            let title = if selected { format!("* {leaf}") } else { leaf };
            let mut row = ListRowSpec::new(format!("file:{p}"), title)
                .with_icon(icon)
                .with_subtitle(p.clone());
            row.selected = selected;
            mark_expanded(row, ws)
        })
        .collect()
}

fn activity_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.terminal_log.is_empty() {
        return vec![
            ListRowSpec::new("act:empty", "No activity").with_icon(ChromeGlyph::Activity.mark())
        ];
    }
    let mut rows: Vec<ListRowSpec> = ws
        .terminal_log
        .iter()
        .enumerate()
        .rev()
        .take(16)
        .map(|(i, line)| {
            mark_expanded(
                ListRowSpec::new(format!("act:{i}"), line.clone())
                    .with_icon(ChromeGlyph::Activity.mark()),
                ws,
            )
        })
        .collect();
    rows.insert(
        0,
        mark_expanded(
            ListRowSpec::new("act:status", if ws.busy { "running" } else { "idle" })
                .with_icon(ChromeGlyph::Activity.mark())
                .with_badge(BadgeSpec::new(
                    if ws.busy { Tone::Warn } else { Tone::Good },
                    if ws.busy { "busy" } else { "idle" },
                )),
            ws,
        ),
    );
    rows
}

fn agent_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.threads.is_empty() {
        return vec![ListRowSpec::new("agent:empty", "No local threads")
            .with_icon(ChromeGlyph::Agent.mark())];
    }
    ws.threads
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut row = ListRowSpec::new(
                format!("agent:{}", t.id),
                crate::persist::thread_leaf_title(&t.title, &t.id),
            )
            .with_icon(ChromeGlyph::Agent.mark())
            .with_subtitle(t.status.clone())
            .with_meta(format!("{} · {} msgs", t.model, t.messages.len()));
            row.selected = i == ws.selected;
            mark_expanded(row, ws)
        })
        .collect()
}

fn diff_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    if ws.diff_rows.is_empty() {
        return vec![ListRowSpec::new("diff:empty", "No working-tree diffs")
            .with_icon(ChromeGlyph::Diff.mark())
            .with_subtitle(ws.diff_sort.label())];
    }
    ws.visible_diffs()
        .into_iter()
        .map(|d| {
            let mut row = ListRowSpec::new(format!("diff:{}", d.path), d.path.clone())
                .with_icon(ChromeGlyph::Diff.mark())
                .with_subtitle(d.status.clone());
            if d.last_turn {
                row = row.with_badge(BadgeSpec::new(Tone::Accent, "turn"));
            }
            row.selected = d.last_turn;
            mark_expanded(row, ws)
        })
        .collect()
}

fn browser_rows(ws: &Workspace) -> Vec<ListRowSpec> {
    let url = if ws.browser_url.is_empty() {
        "(no URL)".into()
    } else {
        ws.browser_url.clone()
    };
    vec![mark_expanded(
        ListRowSpec::new("browser:slot", "System browser")
            .with_icon(ChromeGlyph::Browser.mark())
            .with_subtitle(url)
            .with_meta("CDP later"),
        ws,
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{CoreRow, McpRow, Workspace};

    #[test]
    fn session_rows_include_project() {
        let mut ws = Workspace::new("demo", "grok");
        let rows = inspector_rows(&ws);
        assert!(rows.iter().any(|r| r.id == "session:project"));
        assert!(rows.iter().any(|r| r.subtitle.contains("demo")));
        assert!(rows.iter().any(|r| r.id == "session:turns"));
        ws.usage_turns = 3;
        ws.usage_tokens = 12;
        assert!(row_detail(&ws, "session:turns").contains("Turns"));
        assert!(row_detail(&ws, "session:turns").contains("3"));
        assert_eq!(row_detail(&ws, "session:model"), "grok");
        assert_eq!(row_detail(&ws, "session:connection"), "disconnected");
        assert_eq!(row_detail(&ws, "session:threads"), "1");
    }

    #[test]
    fn diff_rows_sort_and_mark_last_turn() {
        let mut ws = Workspace::new("p", "m");
        ws.inspector = InspectorTab::Diff;
        ws.apply_porcelain(" M zebra.rs\n M alpha.rs\n");
        ws.remember_turn_paths(vec!["zebra.rs".into()]);
        let rows = inspector_rows(&ws);
        assert_eq!(rows[0].id, "diff:zebra.rs");
        assert!(rows[0].selected);
        assert!(row_detail(&ws, "diff:zebra.rs").contains("last turn"));
        ws.set_diff_sort(crate::diff_view::DiffSort::FileName);
        let named = inspector_rows(&ws);
        assert_eq!(named[0].id, "diff:alpha.rs");
        ws.inspector = InspectorTab::Browser;
        let browser = inspector_rows(&ws);
        assert_eq!(browser[0].id, "browser:slot");
        assert!(row_detail(&ws, "browser:slot").contains("CDP/HAR"));
    }

    #[test]
    fn file_rows_hide_collapsed_children() {
        let mut ws = Workspace::new("p", "m");
        ws.inspector = InspectorTab::Files;
        ws.set_files(vec![
            "src/".into(),
            "src/lib.rs".into(),
            "Cargo.toml".into(),
        ]);
        let collapsed = inspector_rows(&ws);
        assert!(collapsed.iter().any(|r| r.id == "file:src/"));
        assert!(collapsed.iter().any(|r| r.id == "file:Cargo.toml"));
        assert!(!collapsed.iter().any(|r| r.id == "file:src/lib.rs"));
        assert!(ws.toggle_file_expand("src/"));
        let open = inspector_rows(&ws);
        assert!(open.iter().any(|r| r.id == "file:src/lib.rs"));
    }

    #[test]
    fn mcp_row_uses_brand_slug_when_known() {
        let mut ws = Workspace::new("p", "m");
        ws.inspector = InspectorTab::Mcp;
        ws.mcp.push(McpRow {
            name: "github".into(),
            command: "npx".into(),
            transport: "stdio".into(),
            state: crate::workspace::McpLife::Stopped,
        });
        let rows = inspector_rows(&ws);
        assert_eq!(rows[0].id, "mcp:github");
        assert_eq!(rows[0].icon, "github-light");
    }

    #[test]
    fn row_detail_mcp_says_no_child() {
        let mut empty = Workspace::new("p", "m");
        empty.inspector = InspectorTab::Mcp;
        let empty_rows = inspector_rows(&empty);
        assert!(empty_rows[0].subtitle.contains("inventory flag (no child)"));
        assert!(!empty_rows[0]
            .subtitle
            .contains("supervised (in-process table)"));

        let mut ws = Workspace::new("p", "m");
        ws.mcp.push(McpRow {
            name: "github".into(),
            command: "npx".into(),
            transport: "stdio".into(),
            state: crate::workspace::McpLife::Stopped,
        });
        let detail = row_detail(&ws, "mcp:github");
        assert!(detail.contains("npx"));
        assert!(detail.contains("stopped"));
        assert!(detail.contains("inventory flag (no child)"));

        ws.inspector = InspectorTab::Mcp;
        let rows = inspector_rows(&ws);
        assert!(rows[0].subtitle.contains("inventory flag (no child)"));
    }

    #[test]
    fn file_row_marks_selected() {
        let mut ws = Workspace::new("p", "m");
        ws.inspector = InspectorTab::Files;
        ws.files.push("src/lib.rs".into());
        ws.files.push("Cargo.toml".into());
        assert!(ws.select_file("src/lib.rs"));
        let rows = inspector_rows(&ws);
        let selected = rows.iter().find(|r| r.id == "file:src/lib.rs").unwrap();
        assert!(selected.selected);
        assert!(selected.title.starts_with("* "));
        let other = rows.iter().find(|r| r.id == "file:Cargo.toml").unwrap();
        assert!(!other.selected);
        assert!(!other.title.starts_with("* "));
        assert!(row_detail(&ws, "file:src/lib.rs").contains("src/lib.rs"));
        assert!(row_detail(&ws, "file:src/lib.rs").contains("selected"));
        assert!(row_detail(&ws, "file:Cargo.toml").contains("Cargo.toml"));
        assert!(row_detail(&ws, "file:Cargo.toml").contains("not selected"));
    }

    #[test]
    fn row_detail_resolves_session_git_point_agent() {
        use crate::workspace::CheckpointRow;
        let mut ws = Workspace::new("demo-proj", "m");
        ws.set_git_status("## main");
        ws.checkpoints.push(CheckpointRow {
            id: "ck1".into(),
            label: "start".into(),
        });
        let tid = ws.threads[0].id.clone();
        assert_eq!(row_detail(&ws, "session:project"), "demo-proj");
        assert_eq!(row_detail(&ws, "git:status"), "## main");
        assert_eq!(row_detail(&ws, "point:ck1"), "start");
        let agent = row_detail(&ws, &format!("agent:{tid}"));
        assert!(agent.contains("New chat"));
        assert!(agent.contains("idle"));
        assert!(row_detail(&ws, "unknown:id").is_empty());
        assert!(row_detail(&ws, "mcp:missing").is_empty());
    }

    #[test]
    fn expand_flag_follows_workspace() {
        let mut ws = Workspace::new("p", "m");
        ws.cores.push(CoreRow {
            index: 0,
            usage: 10.0,
            reserved: true,
        });
        ws.inspector = InspectorTab::Resources;
        ws.toggle_right_row("core:0");
        let rows = inspector_rows(&ws);
        assert!(rows.iter().any(|r| r.id == "core:0" && r.expanded));
    }
}
