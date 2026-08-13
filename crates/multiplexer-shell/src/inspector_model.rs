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
    }
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
    if ws.mcp.is_empty() {
        return vec![ListRowSpec::new("mcp:empty", "No MCP servers")
            .with_icon(ChromeGlyph::Plug.mark())
            .with_subtitle("~/.grok/config.toml")];
    }
    ws.mcp
        .iter()
        .map(|m| {
            let icon = BrandIcon::from_name(&m.name)
                .or_else(|| BrandIcon::from_name(&m.command))
                .map(|b| b.slug().to_string())
                .unwrap_or_else(|| ChromeGlyph::Plug.mark().to_string());
            let tone = match m.state {
                crate::workspace::McpLife::Ready => Tone::Good,
                crate::workspace::McpLife::Crashed | crate::workspace::McpLife::Failed => {
                    Tone::Danger
                }
                crate::workspace::McpLife::Stopped => Tone::Neutral,
            };
            mark_expanded(
                ListRowSpec::new(format!("mcp:{}", m.name), m.name.clone())
                    .with_icon(icon)
                    .with_subtitle(m.command.clone())
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
    if ws.skills.is_empty() {
        return vec![ListRowSpec::new("skill:empty", "No skills")
            .with_icon(ChromeGlyph::Sparkle.mark())
            .with_subtitle(".grok/skills")];
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
    ws.files
        .iter()
        .map(|p| {
            let icon = if p.ends_with('/') {
                ChromeGlyph::Folder.mark()
            } else {
                ChromeGlyph::Diff.mark()
            };
            mark_expanded(
                ListRowSpec::new(format!("file:{p}"), p.clone()).with_icon(icon),
                ws,
            )
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
            let mut row = ListRowSpec::new(format!("agent:{}", t.id), t.title.clone())
                .with_icon(ChromeGlyph::Agent.mark())
                .with_subtitle(t.status.clone())
                .with_meta(format!("{} msgs", t.messages.len()));
            row.selected = i == ws.selected;
            mark_expanded(row, ws)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{CoreRow, McpRow, Workspace};

    #[test]
    fn session_rows_include_project() {
        let ws = Workspace::new("demo", "grok");
        let rows = inspector_rows(&ws);
        assert!(rows.iter().any(|r| r.id == "session:project"));
        assert!(rows.iter().any(|r| r.subtitle.contains("demo")));
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
