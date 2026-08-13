//! Right-rail inspector: labeled tab buttons plus body text.

use multiplexer_shell::{InspectorTab, Workspace};

/// Host-handled action for one inspector button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorAction {
    RefreshCores,
    RefreshMcp,
    RefreshGit,
    CreateCheckpoint,
    RevertCheckpoint,
    CycleModel,
    CopySession,
    RunGitStatus,
    NewWorktreeHint,
}

/// One labeled control for the active inspector tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectorButton {
    pub label: &'static str,
    pub hint: &'static str,
    pub action: InspectorAction,
}

fn button(label: &'static str, hint: &'static str, action: InspectorAction) -> InspectorButton {
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
            button(
                "Model",
                "Cycle the session model",
                InspectorAction::CycleModel,
            ),
            button("Copy", "Copy the session id", InspectorAction::CopySession),
        ],
        InspectorTab::Resources => vec![button(
            "Reload",
            "Refresh reserved cores",
            InspectorAction::RefreshCores,
        )],
        InspectorTab::Mcp => vec![button(
            "Reload",
            "Refresh MCP inventory",
            InspectorAction::RefreshMcp,
        )],
        InspectorTab::Checkpoints => vec![
            button(
                "New",
                "Create a checkpoint",
                InspectorAction::CreateCheckpoint,
            ),
            button(
                "Revert",
                "Revert to a checkpoint",
                InspectorAction::RevertCheckpoint,
            ),
        ],
        InspectorTab::Git => vec![
            button("Reload", "Refresh worktrees", InspectorAction::RefreshGit),
            button("Status", "Run git status", InspectorAction::RunGitStatus),
            button(
                "New WT",
                "Hint a worktree path",
                InspectorAction::NewWorktreeHint,
            ),
        ],
        InspectorTab::Terminal | InspectorTab::Skills => Vec::new(),
        InspectorTab::Files => Vec::new(),
        InspectorTab::Activity => Vec::new(),
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
            .any(|b| b.label == "Model" && b.action == InspectorAction::CycleModel));
        assert!(buttons
            .iter()
            .any(|b| b.label == "Copy" && b.action == InspectorAction::CopySession));
    }

    #[test]
    fn tab_buttons_checkpoints_has_new() {
        let buttons = tab_buttons(InspectorTab::Checkpoints);
        assert!(buttons
            .iter()
            .any(|b| b.label == "New" && b.action == InspectorAction::CreateCheckpoint));
        assert!(buttons
            .iter()
            .any(|b| b.label == "Revert" && b.action == InspectorAction::RevertCheckpoint));
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
    }
}
