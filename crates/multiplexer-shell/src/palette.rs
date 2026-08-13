//! Command palette catalog and filter state.
//!
//! InspectorTab has no Git/Terminal/Skills variants, so those slash targets
//! have no SelectTab rows. RunTerminal and RefreshGit cover those hosts.

use crate::workspace::InspectorTab;
use crate::ClientAction;

/// One searchable row in the command palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteItem {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub action: crate::ClientAction,
}

/// Built-in palette rows for every static [`ClientAction`] the chrome can fire.
pub fn default_items() -> Vec<PaletteItem> {
    vec![
        PaletteItem {
            id: "new-chat",
            label: "New chat",
            hint: "Ctrl+N",
            action: ClientAction::NewThread,
        },
        PaletteItem {
            id: "toggle-chats",
            label: "Toggle chats",
            hint: "Ctrl+[",
            action: ClientAction::ToggleLeft,
        },
        PaletteItem {
            id: "toggle-inspector",
            label: "Toggle inspector",
            hint: "Ctrl+]",
            action: ClientAction::ToggleRight,
        },
        PaletteItem {
            id: "toggle-terminal",
            label: "Toggle terminal",
            hint: "Ctrl+`",
            action: ClientAction::ToggleBottom,
        },
        PaletteItem {
            id: "send",
            label: "Send",
            hint: "Enter",
            action: ClientAction::Send,
        },
        PaletteItem {
            id: "stop",
            label: "Stop",
            hint: "Ctrl+.",
            action: ClientAction::Interrupt,
        },
        PaletteItem {
            id: "checkpoint",
            label: "Create checkpoint",
            hint: "Ctrl+S",
            action: ClientAction::CreateCheckpoint,
        },
        PaletteItem {
            id: "cores",
            label: "Cores",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Resources),
        },
        PaletteItem {
            id: "mcp",
            label: "MCP",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Mcp),
        },
        PaletteItem {
            id: "points",
            label: "Points",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Checkpoints),
        },
        PaletteItem {
            id: "git-tab",
            label: "Git",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Git),
        },
        PaletteItem {
            id: "term-tab",
            label: "Terminal",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Terminal),
        },
        PaletteItem {
            id: "skills-tab",
            label: "Skills",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Skills),
        },
        PaletteItem {
            id: "session",
            label: "Session",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Session),
        },
        PaletteItem {
            id: "dismiss-reminder",
            label: "Dismiss reminder",
            hint: "",
            action: ClientAction::DismissReminder,
        },
        PaletteItem {
            id: "refresh-cores",
            label: "Refresh cores",
            hint: "",
            action: ClientAction::RefreshCores,
        },
        PaletteItem {
            id: "refresh-mcp",
            label: "Refresh MCP",
            hint: "",
            action: ClientAction::RefreshMcp,
        },
        PaletteItem {
            id: "help",
            label: "Toggle help",
            hint: "F1",
            action: ClientAction::ToggleHelp,
        },
        PaletteItem {
            id: "close-palette",
            label: "Close palette",
            hint: "Esc",
            action: ClientAction::ClosePalette,
        },
        PaletteItem {
            id: "toggle-palette",
            label: "Toggle palette",
            hint: "Ctrl+K · Ctrl+Shift+P",
            action: ClientAction::TogglePalette,
        },
        PaletteItem {
            id: "delete-thread",
            label: "Delete thread",
            hint: "",
            action: ClientAction::DeleteThread,
        },
        PaletteItem {
            id: "cycle-model",
            label: "Cycle model",
            hint: "",
            action: ClientAction::CycleModel,
        },
        PaletteItem {
            id: "approve",
            label: "Approve",
            hint: "",
            action: ClientAction::Approve,
        },
        PaletteItem {
            id: "deny",
            label: "Deny",
            hint: "",
            action: ClientAction::Deny,
        },
        PaletteItem {
            id: "restore-checkpoint",
            label: "Restore files to checkpoint",
            hint: "",
            action: ClientAction::RestoreCheckpoint,
        },
        PaletteItem {
            id: "run-terminal",
            label: "Run terminal",
            hint: "",
            action: ClientAction::RunTerminal,
        },
        PaletteItem {
            id: "refresh-git",
            label: "Refresh git",
            hint: "",
            action: ClientAction::RefreshGit,
        },
        PaletteItem {
            id: "cycle-file",
            label: "Cycle file",
            hint: "",
            action: ClientAction::CycleFile,
        },
        PaletteItem {
            id: "copy-last-message",
            label: "Copy last message",
            hint: "",
            action: ClientAction::CopyLastMessage,
        },
        PaletteItem {
            id: "files-tab",
            label: "Files",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Files),
        },
        PaletteItem {
            id: "activity-tab",
            label: "Activity",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Activity),
        },
        PaletteItem {
            id: "agents-tab",
            label: "Agents",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Agents),
        },
        PaletteItem {
            id: "settings",
            label: "Settings",
            hint: "Ctrl+,",
            action: ClientAction::ToggleSettings,
        },
        PaletteItem {
            id: "search",
            label: "Search names",
            hint: "Ctrl+P",
            action: ClientAction::ToggleSearch,
        },
        PaletteItem {
            id: "focus-layout",
            label: "Focus layout",
            hint: "Ctrl+Shift+H",
            action: ClientAction::FocusLayout,
        },
        PaletteItem {
            id: "create-worktree",
            label: "Create worktree",
            hint: "",
            action: ClientAction::CreateWorktree,
        },
        PaletteItem {
            id: "mention-file",
            label: "Mention selected file",
            hint: "",
            action: ClientAction::InsertFileMention,
        },
        PaletteItem {
            id: "center-tui",
            label: "Grok TUI",
            hint: "Ctrl+Shift+G",
            action: ClientAction::SetCenterTui,
        },
        PaletteItem {
            id: "center-gui",
            label: "Chat log",
            hint: "",
            action: ClientAction::SetCenterGui,
        },
        PaletteItem {
            id: "launch-tui",
            label: "Launch Grok TUI",
            hint: "",
            action: ClientAction::LaunchGrokTui,
        },
        PaletteItem {
            id: "diff-tab",
            label: "Diffs",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Diff),
        },
        PaletteItem {
            id: "browser-tab",
            label: "Browser",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Browser),
        },
        PaletteItem {
            id: "popout-inspector",
            label: "Pop out inspector",
            hint: "Ctrl+Shift+D",
            action: ClientAction::PopOutInspector,
        },
        PaletteItem {
            id: "dock-inspector",
            label: "Dock inspector",
            hint: "Ctrl+Shift+E",
            action: ClientAction::DockInspector,
        },
        PaletteItem {
            id: "about",
            label: "About Multiplexer",
            hint: "",
            action: ClientAction::OpenAbout,
        },
    ]
}

/// Case-insensitive substring match on `id`, `label`, or `hint`.
///
/// An empty query returns the full catalog.
pub fn filter_items(query: &str) -> Vec<PaletteItem> {
    let items = default_items();
    if query.is_empty() {
        return items;
    }
    let needle = query.to_lowercase();
    items
        .into_iter()
        .filter(|item| {
            item.id.to_lowercase().contains(&needle)
                || item.label.to_lowercase().contains(&needle)
                || item.hint.to_lowercase().contains(&needle)
        })
        .collect()
}

/// Destination panes. Empty palette query includes these; never the file tree.
pub fn pane_items() -> Vec<PaletteItem> {
    use crate::workspace::{InspectorTab, LeftSection};
    vec![
        PaletteItem {
            id: "pane-chats",
            label: "Chats",
            hint: "Ctrl+1",
            action: ClientAction::SelectLeftSection(LeftSection::Threads),
        },
        PaletteItem {
            id: "pane-agents",
            label: "Agents",
            hint: "Ctrl+2",
            action: ClientAction::SelectLeftSection(LeftSection::Agents),
        },
        PaletteItem {
            id: "pane-files",
            label: "Files",
            hint: "Ctrl+3",
            action: ClientAction::SelectLeftSection(LeftSection::Files),
        },
        PaletteItem {
            id: "pane-activity",
            label: "Activity",
            hint: "Ctrl+4",
            action: ClientAction::SelectLeftSection(LeftSection::Activity),
        },
        PaletteItem {
            id: "pane-session",
            label: "Session",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Session),
        },
        PaletteItem {
            id: "pane-git",
            label: "Git",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Git),
        },
        PaletteItem {
            id: "pane-diffs",
            label: "Diffs",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Diff),
        },
        PaletteItem {
            id: "pane-browser",
            label: "Browser",
            hint: "",
            action: ClientAction::SelectTab(InspectorTab::Browser),
        },
        PaletteItem {
            id: "pane-term",
            label: "Terminal drawer",
            hint: "Ctrl+`",
            action: ClientAction::ToggleBottom,
        },
    ]
}

fn hit_from_item(kind: crate::SearchKind, item: &PaletteItem) -> crate::SearchHit {
    crate::SearchHit {
        kind,
        id: item.id.to_owned(),
        title: item.label.to_owned(),
        hint: item.hint.to_owned(),
    }
}

/// Palette rows. Empty query is recent + panes + commands (no files).
/// A nonempty query is a fuzzy subsequence across namespaces.
pub fn palette_hits(ws: &crate::Workspace, query: &str) -> Vec<crate::SearchHit> {
    if query.trim().is_empty() {
        let mut hits = Vec::new();
        for id in ws.recent_commands.iter().take(8) {
            if let Some(item) = default_items().into_iter().find(|i| i.id == id.as_str()) {
                hits.push(hit_from_item(crate::SearchKind::Recent, &item));
            }
        }
        for item in pane_items() {
            hits.push(hit_from_item(crate::SearchKind::Pane, &item));
        }
        for item in default_items() {
            hits.push(hit_from_item(crate::SearchKind::Command, &item));
        }
        return hits;
    }
    let mut scored: Vec<(u32, crate::SearchHit)> = Vec::new();
    for t in &ws.threads {
        if let Some(score) = crate::fuzzy::fuzzy_best(query, &[&t.title, &t.id]) {
            scored.push((
                score,
                crate::SearchHit {
                    kind: crate::SearchKind::Thread,
                    id: t.id.clone(),
                    title: t.title.clone(),
                    hint: "thread".into(),
                },
            ));
        }
    }
    for f in &ws.files {
        if let Some(score) = crate::fuzzy::fuzzy_score(query, f) {
            scored.push((
                score,
                crate::SearchHit {
                    kind: crate::SearchKind::File,
                    id: f.clone(),
                    title: f.clone(),
                    hint: "file".into(),
                },
            ));
        }
    }
    for item in pane_items() {
        if let Some(score) = crate::fuzzy::fuzzy_best(query, &[item.id, item.label, item.hint]) {
            scored.push((score, hit_from_item(crate::SearchKind::Pane, &item)));
        }
    }
    for item in default_items() {
        if let Some(score) = crate::fuzzy::fuzzy_best(query, &[item.id, item.label, item.hint]) {
            scored.push((score, hit_from_item(crate::SearchKind::Command, &item)));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.title.cmp(&b.1.title)));
    scored.into_iter().map(|(_, h)| h).collect()
}

/// Resolve a palette hit to the action that should run.
pub fn hit_action(ws: &crate::Workspace, hit: &crate::SearchHit) -> Option<ClientAction> {
    match hit.kind {
        crate::SearchKind::Thread => ws
            .threads
            .iter()
            .position(|t| t.id == hit.id)
            .map(ClientAction::SelectThread),
        crate::SearchKind::File => Some(ClientAction::SelectTab(
            crate::workspace::InspectorTab::Files,
        )),
        crate::SearchKind::Command | crate::SearchKind::Recent => default_items()
            .into_iter()
            .find(|i| i.id == hit.id)
            .map(|i| i.action),
        crate::SearchKind::Pane => pane_items()
            .into_iter()
            .find(|i| i.id == hit.id)
            .map(|i| i.action),
    }
}

/// Open/query/selection state for the palette overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteState {
    pub open: bool,
    pub query: String,
    pub selected: usize,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self::new()
    }
}

impl PaletteState {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            selected: 0,
        }
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open = true;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected = 0;
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.selected = 0;
    }

    pub fn move_up(&mut self) {
        let n = filter_items(&self.query).len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        let cur = if self.selected >= n { 0 } else { self.selected };
        self.selected = (cur + n - 1) % n;
    }

    pub fn move_down(&mut self) {
        let n = filter_items(&self.query).len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        let cur = if self.selected >= n { 0 } else { self.selected };
        self.selected = (cur + 1) % n;
    }

    pub fn active_item(&self) -> Option<PaletteItem> {
        let items = filter_items(&self.query);
        if items.is_empty() {
            None
        } else {
            items.get(self.selected.min(items.len() - 1)).copied()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn require(id: &str) -> PaletteItem {
        default_items()
            .into_iter()
            .find(|item| item.id == id)
            .unwrap_or_else(|| panic!("missing palette item {id}"))
    }

    #[test]
    fn default_has_new_chat() {
        let item = require("new-chat");
        assert_eq!(item.label, "New chat");
        assert_eq!(item.hint, "Ctrl+N");
        assert_eq!(item.action, ClientAction::NewThread);
        assert_eq!(default_items()[0].id, "new-chat");
    }

    #[test]
    fn default_catalog_covers_required_ids() {
        let expected: &[(&str, &str, ClientAction)] = &[
            ("new-chat", "Ctrl+N", ClientAction::NewThread),
            ("toggle-chats", "Ctrl+[", ClientAction::ToggleLeft),
            ("toggle-inspector", "Ctrl+]", ClientAction::ToggleRight),
            ("toggle-terminal", "Ctrl+`", ClientAction::ToggleBottom),
            ("send", "Enter", ClientAction::Send),
            ("stop", "Ctrl+.", ClientAction::Interrupt),
            ("checkpoint", "Ctrl+S", ClientAction::CreateCheckpoint),
            (
                "cores",
                "",
                ClientAction::SelectTab(InspectorTab::Resources),
            ),
            ("mcp", "", ClientAction::SelectTab(InspectorTab::Mcp)),
            (
                "points",
                "",
                ClientAction::SelectTab(InspectorTab::Checkpoints),
            ),
            ("help", "F1", ClientAction::ToggleHelp),
            ("close-palette", "Esc", ClientAction::ClosePalette),
        ];
        let items = default_items();
        for (id, hint, action) in expected {
            let item = items
                .iter()
                .find(|row| row.id == *id)
                .unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(item.hint, *hint, "{id} hint");
            assert_eq!(item.action, *action, "{id} action");
        }
        let ids: Vec<_> = items.iter().map(|item| item.id).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(ids.len(), unique.len(), "duplicate palette ids: {ids:?}");
        let term = require("toggle-terminal");
        assert_eq!(term.label, "Toggle terminal");
        assert_eq!(term.hint, "Ctrl+`");
        assert_eq!(term.action, ClientAction::ToggleBottom);
    }

    #[test]
    fn filter_narrows() {
        let all = filter_items("");
        assert_eq!(all.len(), default_items().len());
        assert!(all.len() >= 9);

        let mcp = filter_items("mcp");
        assert!(mcp.iter().any(|item| item.id == "mcp"));
        assert!(mcp.len() < all.len());
        assert!(mcp.iter().all(|item| {
            item.id.to_lowercase().contains("mcp")
                || item.label.to_lowercase().contains("mcp")
                || item.hint.to_lowercase().contains("mcp")
        }));

        assert!(filter_items("zzz-no-such-command").is_empty());
        let by_id = filter_items("NEW-CHAT");
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].id, "new-chat");
    }

    #[test]
    fn filter_matches_id_label_or_hint() {
        let by_hint = filter_items("ctrl+n");
        assert_eq!(by_hint.len(), 1);
        assert_eq!(by_hint[0].id, "new-chat");

        let by_label = filter_items("toggle inspector");
        assert_eq!(by_label.len(), 1);
        assert_eq!(by_label[0].id, "toggle-inspector");

        let by_id = filter_items("toggle-chats");
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].action, ClientAction::ToggleLeft);
    }

    #[test]
    fn move_wraps() {
        let mut state = PaletteState::new();
        let n = filter_items(&state.query).len();
        assert!(n >= 2);
        assert_eq!(state.selected, 0);

        state.move_up();
        assert_eq!(state.selected, n - 1);
        assert_eq!(
            state.active_item().map(|item| item.id),
            Some(filter_items("")[n - 1].id)
        );

        state.move_down();
        assert_eq!(state.selected, 0);
        assert_eq!(state.active_item().unwrap().id, "new-chat");

        for _ in 0..n {
            state.move_down();
        }
        assert_eq!(state.selected, 0);

        state.selected = n + 3;
        state.move_down();
        assert_eq!(state.selected, 1);
        state.selected = n + 3;
        state.move_up();
        assert_eq!(state.selected, n - 1);
    }

    #[test]
    fn close_clears_query() {
        let mut state = PaletteState::new();
        state.toggle();
        assert!(state.open);
        state.set_query("mcp");
        state.selected = 2;
        assert!(!state.query.is_empty());
        assert_ne!(state.selected, 0);

        state.close();
        assert!(!state.open);
        assert!(state.query.is_empty());
        assert_eq!(state.selected, 0);

        state.toggle();
        state.set_query("points");
        state.selected = 2;
        state.toggle();
        assert!(!state.open);
        assert!(state.query.is_empty());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn set_query_resets_selected_and_active_item() {
        let mut state = PaletteState::new();
        state.selected = 4;
        state.set_query("points");
        assert_eq!(state.selected, 0);
        let active = state.active_item().expect("points row");
        assert_eq!(active.id, "points");
        assert_eq!(
            active.action,
            ClientAction::SelectTab(InspectorTab::Checkpoints)
        );

        state.set_query("zzz-no-such-command");
        assert_eq!(state.selected, 0);
        assert_eq!(state.active_item(), None);
        state.move_down();
        state.move_up();
        assert_eq!(state.selected, 0);
        assert_eq!(state.active_item(), None);
    }

    #[test]
    fn default_state_is_closed() {
        let state = PaletteState::new();
        assert_eq!(state, PaletteState::default());
        assert!(!state.open);
        assert!(state.query.is_empty());
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn palette_filter_includes_files_and_threads() {
        let mut ws = crate::Workspace::new("p", "m");
        ws.threads[0].title = "Fix search".into();
        ws.set_files(vec!["src/main.rs".into()]);

        let search_hits = palette_hits(&ws, "search");
        assert!(search_hits
            .iter()
            .any(|h| { h.kind == crate::SearchKind::Thread && h.title == "Fix search" }));

        let file_hits = palette_hits(&ws, "main.rs");
        assert!(file_hits
            .iter()
            .any(|h| h.kind == crate::SearchKind::File && h.title.contains("main.rs")));

        let empty = palette_hits(&ws, "");
        assert!(empty.iter().any(|h| h.kind == crate::SearchKind::Command));
        assert!(empty.iter().any(|h| h.kind == crate::SearchKind::Pane));
        assert!(empty.iter().all(|h| h.kind != crate::SearchKind::File));
    }

    #[test]
    fn empty_includes_recent_and_fuzzy_ranks() {
        let mut ws = crate::Workspace::new("p", "m");
        ws.remember_command("mcp");
        ws.remember_command("new-chat");
        let empty = palette_hits(&ws, "");
        assert_eq!(empty[0].kind, crate::SearchKind::Recent);
        assert_eq!(empty[0].id, "new-chat");
        assert!(empty
            .iter()
            .any(|h| h.kind == crate::SearchKind::Recent && h.id == "mcp"));

        let hits = palette_hits(&ws, "cp");
        assert!(hits.iter().any(|h| h.id == "checkpoint"));
        assert!(
            hit_action(&ws, hits.iter().find(|h| h.id == "checkpoint").unwrap())
                .is_some_and(|a| a == ClientAction::CreateCheckpoint)
        );
        let pane = empty.iter().find(|h| h.id == "pane-git").unwrap();
        assert_eq!(
            hit_action(&ws, pane),
            Some(ClientAction::SelectTab(InspectorTab::Git))
        );
    }
}
