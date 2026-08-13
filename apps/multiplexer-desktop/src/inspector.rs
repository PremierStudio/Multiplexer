//! Right-rail inspector: labeled tab buttons plus body text.

use multiplexer_shell::{ClientAction, InspectorTab, Workspace};

/// One labeled control for the active inspector tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectorButton {
    pub label: &'static str,
    pub hint: &'static str,
    pub action: ClientAction,
}

fn button(label: &'static str, hint: &'static str, action: ClientAction) -> InspectorButton {
    InspectorButton {
        label,
        hint,
        action,
    }
}

/// Labeled buttons for `tab` so the GPUI rail can render real controls.
pub fn tab_buttons(tab: InspectorTab) -> Vec<InspectorButton> {
    match tab {
        InspectorTab::Session => vec![
            button("Model", "Cycle the session model", ClientAction::CycleModel),
            button("Copy", "Copy the session id", ClientAction::CopySession),
        ],
        InspectorTab::Resources => vec![button(
            "Reload",
            "Refresh reserved cores",
            ClientAction::RefreshCores,
        )],
        InspectorTab::Mcp => vec![
            button("Reload", "Refresh MCP inventory", ClientAction::RefreshMcp),
            button("Start", "Set ready flag (no child)", ClientAction::StartMcp),
            button("Stop", "Clear ready flag", ClientAction::StopMcp),
        ],
        InspectorTab::Checkpoints => vec![
            button("New", "Create a checkpoint", ClientAction::CreateCheckpoint),
            button(
                "Set pointer",
                "Move pointer only, files unchanged",
                ClientAction::RestoreCheckpoint,
            ),
        ],
        InspectorTab::Git => vec![
            button("Reload", "Refresh worktrees", ClientAction::RefreshGit),
            button("Status", "Run git status", ClientAction::RunGitStatus),
            button(
                "Create",
                "git.worktree.create",
                ClientAction::CreateWorktree,
            ),
            button(
                "Switch",
                "use selected worktree cwd",
                ClientAction::SwitchWorktree,
            ),
            button(
                "Remove",
                "remove selected worktree",
                ClientAction::RemoveWorktree,
            ),
        ],
        InspectorTab::Terminal => vec![button(
            "Kill",
            "kill the running command",
            ClientAction::KillTerm,
        )],
        InspectorTab::Skills => Vec::new(),
        InspectorTab::Files => vec![
            button("Reload", "rescan project tree", ClientAction::RefreshFiles),
            button("Reveal", "copy absolute path", ClientAction::RevealFile),
            button("Open", "open in system app", ClientAction::OpenExternal),
            button(
                "Mention",
                "@ path into composer",
                ClientAction::InsertFileMention,
            ),
        ],
        InspectorTab::Activity => Vec::new(),
        InspectorTab::Agents => Vec::new(),
        InspectorTab::Diff => vec![
            button(
                "Reload",
                "git status --porcelain",
                ClientAction::ReloadDiffs,
            ),
            button(
                "Last turn",
                "sort last turn first",
                ClientAction::SortDiffLastTurn,
            ),
            button("Name", "sort by file name", ClientAction::SortDiffFileName),
            button(
                "Mention",
                "@ selected diff path",
                ClientAction::InsertFileMention,
            ),
        ],
        InspectorTab::Browser => vec![button("Open", "system browser", ClientAction::OpenBrowser)],
    }
}

/// Copy for the active inspector tab. Tests and expanded-row fallbacks use this.
#[allow(dead_code)]
pub fn inspector_body(ws: &Workspace, session_id: Option<&str>) -> String {
    match ws.inspector {
        InspectorTab::Session => ws.session_detail(session_id),
        InspectorTab::Resources => ws.resource_detail(),
        InspectorTab::Mcp => ws.mcp_detail(),
        InspectorTab::Checkpoints => ws.checkpoint_detail(),
        InspectorTab::Git => ws.git_detail(),
        InspectorTab::Terminal => ws.terminal_detail(),
        InspectorTab::Skills => ws.skills_detail(),
        InspectorTab::Files => ws.files_detail(),
        InspectorTab::Activity => ws.activity_detail(),
        InspectorTab::Agents => ws.agents_detail(),
        InspectorTab::Diff => ws.diff_detail(),
        InspectorTab::Browser => ws.browser_detail(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_tab_contains_project() {
        let ws = Workspace::new("demo", "grok");
        assert_eq!(ws.inspector, InspectorTab::Session);
        assert!(inspector_body(&ws, None).contains("Project"));
    }

    #[test]
    fn tab_buttons_session_has_model() {
        let buttons = tab_buttons(InspectorTab::Session);
        assert!(buttons
            .iter()
            .any(|b| b.label == "Model" && b.action == ClientAction::CycleModel));
        assert!(buttons
            .iter()
            .any(|b| b.label == "Copy" && b.action == ClientAction::CopySession));
    }

    #[test]
    fn tab_buttons_checkpoints_has_new() {
        let buttons = tab_buttons(InspectorTab::Checkpoints);
        assert!(buttons
            .iter()
            .any(|b| b.label == "New" && b.action == ClientAction::CreateCheckpoint));
        assert!(buttons
            .iter()
            .any(|b| b.label == "Set pointer" && b.action == ClientAction::RestoreCheckpoint));
    }

    #[test]
    fn inspector_body_matches_tab() {
        let mut ws = Workspace::new("demo", "grok");
        assert_eq!(ws.inspector, InspectorTab::Session);
        assert_eq!(inspector_body(&ws, None), ws.session_detail(None));
        assert_eq!(
            inspector_body(&ws, Some("sess-1")),
            ws.session_detail(Some("sess-1"))
        );

        ws.inspector = InspectorTab::Resources;
        assert_eq!(inspector_body(&ws, None), ws.resource_detail());

        ws.inspector = InspectorTab::Mcp;
        assert_eq!(inspector_body(&ws, None), ws.mcp_detail());

        ws.inspector = InspectorTab::Checkpoints;
        assert_eq!(inspector_body(&ws, None), ws.checkpoint_detail());

        ws.inspector = InspectorTab::Git;
        assert_eq!(inspector_body(&ws, None), ws.git_detail());
        assert!(!tab_buttons(InspectorTab::Git).is_empty());

        ws.inspector = InspectorTab::Terminal;
        assert_eq!(inspector_body(&ws, None), ws.terminal_detail());

        ws.inspector = InspectorTab::Skills;
        assert_eq!(inspector_body(&ws, None), ws.skills_detail());
        ws.inspector = InspectorTab::Files;
        assert_eq!(inspector_body(&ws, None), ws.files_detail());
        ws.inspector = InspectorTab::Activity;
        assert_eq!(inspector_body(&ws, None), ws.activity_detail());
        ws.inspector = InspectorTab::Agents;
        assert_eq!(inspector_body(&ws, None), ws.agents_detail());
        ws.inspector = InspectorTab::Diff;
        assert_eq!(inspector_body(&ws, None), ws.diff_detail());
        assert_eq!(tab_buttons(InspectorTab::Diff).len(), 4);
        ws.inspector = InspectorTab::Browser;
        assert_eq!(inspector_body(&ws, None), ws.browser_detail());
        assert!(!tab_buttons(InspectorTab::Browser).is_empty());
    }
}
