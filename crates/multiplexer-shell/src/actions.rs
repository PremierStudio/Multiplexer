//! Client chrome actions. Layout mutations live here; I/O stays in the host.

use crate::workspace::{InspectorTab, Workspace};

/// A user gesture the desktop can dispatch into the headless workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAction {
    NewThread,
    SelectThread(usize),
    ToggleLeft,
    ToggleRight,
    SelectTab(InspectorTab),
    Send,
    Interrupt,
    DismissReminder,
    RefreshCores,
    RefreshMcp,
    CreateCheckpoint,
    DeleteThread,
    CycleModel,
    TogglePalette,
    ClosePalette,
    ToggleHelp,
    Approve,
    Deny,
    RestoreCheckpoint,
    RunTerminal,
    RefreshGit,
    CycleFile,
    CopyLastMessage,
    CreateWorktree,
    SelectModel,
    SelectLeftSection(crate::workspace::LeftSection),
    ToggleBottom,
    StartMcp,
    StopMcp,
    InsertFileMention,
    ToggleSettings,
    ToggleCenterMode,
    SetCenterGui,
    SetCenterTui,
    LaunchGrokTui,
    StopGrokTui,
    SortDiffLastTurn,
    SortDiffFileName,
    OpenBrowser,
    HideLeft,
    HideRight,
    HideBottom,
    FocusLayout,
    OpenProjectFiles,
    OpenGitTab,
    OpenSessionTab,
    OpenSettingsRemotes,
    ToggleSearch,
    CloseSearch,
    CloseOverlay,
    ResetOutlook,
    CopySession,
    ReloadDiffs,
    RunGitStatus,
    DismissToast,
}

/// Apply a workspace-only layout action.
///
/// Returns true when `ws` changed. Host-owned I/O actions do not mutate here:
/// Send, Interrupt, RefreshCores, RefreshMcp, CreateCheckpoint, Approve, Deny,
/// RestoreCheckpoint, RunTerminal, RefreshGit, CycleFile, CopyLastMessage.
pub fn apply_layout_action(ws: &mut Workspace, action: ClientAction) -> bool {
    match action {
        ClientAction::NewThread => {
            ws.new_thread();
            true
        }
        ClientAction::SelectThread(index) => {
            let before = ws.selected;
            let _ = ws.select(index);
            ws.selected != before
        }
        ClientAction::ToggleLeft => {
            ws.chrome.toggle_left();
            true
        }
        ClientAction::ToggleRight => {
            ws.chrome.toggle_right();
            true
        }
        ClientAction::SelectTab(tab) => {
            let changed = ws.select_inspector(tab);
            if !ws.chrome.right_open() {
                ws.chrome.open_right();
                true
            } else {
                changed
            }
        }
        ClientAction::SelectLeftSection(section) => {
            let changed = ws.select_left_section(section);
            if !ws.chrome.left_open() {
                ws.chrome.open_left();
                true
            } else {
                changed
            }
        }
        ClientAction::HideLeft => {
            if ws.chrome.left == crate::workspace::RailVis::Hidden {
                false
            } else {
                ws.chrome.hide_left();
                true
            }
        }
        ClientAction::HideRight => {
            if ws.chrome.right == crate::workspace::RailVis::Hidden {
                false
            } else {
                ws.chrome.hide_right();
                true
            }
        }
        ClientAction::HideBottom => {
            if ws.bottom_hidden {
                false
            } else {
                ws.hide_bottom();
                true
            }
        }
        ClientAction::FocusLayout => ws.focus_layout(),
        ClientAction::OpenProjectFiles => {
            let sec = ws.select_left_section(crate::workspace::LeftSection::Files);
            let tab = ws.select_inspector(crate::workspace::InspectorTab::Files);
            ws.chrome.open_left();
            ws.chrome.open_right();
            sec || tab
        }
        ClientAction::OpenGitTab => {
            let changed = ws.select_inspector(crate::workspace::InspectorTab::Git);
            let was_closed = !ws.chrome.right_open();
            ws.chrome.open_right();
            changed || was_closed
        }
        ClientAction::OpenSessionTab => {
            let changed = ws.select_inspector(crate::workspace::InspectorTab::Session);
            let was_closed = !ws.chrome.right_open();
            ws.chrome.open_right();
            changed || was_closed
        }
        ClientAction::OpenSettingsRemotes => {
            let changed = !ws.settings_open
                || ws.settings_section != crate::settings::SettingsSection::Remotes;
            ws.settings_section = crate::settings::SettingsSection::Remotes;
            ws.open_overlay(crate::overlay::OverlayKind::Settings);
            changed
        }
        ClientAction::ToggleBottom => {
            ws.toggle_bottom();
            true
        }
        ClientAction::ToggleCenterMode => {
            ws.toggle_center_mode();
            true
        }
        ClientAction::SetCenterGui => ws.set_center_mode(crate::center::CenterMode::Gui),
        ClientAction::SetCenterTui => ws.set_center_mode(crate::center::CenterMode::GrokTui),
        ClientAction::SortDiffLastTurn => ws.set_diff_sort(crate::diff_view::DiffSort::LastTurn),
        ClientAction::SortDiffFileName => ws.set_diff_sort(crate::diff_view::DiffSort::FileName),
        ClientAction::CreateWorktree => false,
        ClientAction::SelectModel => {
            let model = ws.settings.default_model.clone();
            ws.select_model(model)
        }
        ClientAction::InsertFileMention => ws.insert_file_mention(),
        ClientAction::ToggleSettings => {
            ws.toggle_overlay(crate::overlay::OverlayKind::Settings);
            true
        }
        ClientAction::ToggleSearch => {
            ws.toggle_overlay(crate::overlay::OverlayKind::Search);
            true
        }
        ClientAction::CloseSearch => {
            if !ws.search_open {
                false
            } else {
                ws.close_overlay(crate::overlay::OverlayKind::Search);
                true
            }
        }
        ClientAction::CloseOverlay => ws.pop_overlay().is_some(),
        ClientAction::ResetOutlook => {
            ws.reset_outlook_chrome();
            true
        }
        ClientAction::DismissToast => ws.dismiss_newest_notice(),
        ClientAction::StartMcp => {
            let name = ws
                .right_expanded_id
                .as_deref()
                .and_then(|id| id.strip_prefix("mcp:"))
                .map(str::to_owned);
            name.map(|n| ws.start_mcp(&n)).unwrap_or(false)
        }
        ClientAction::StopMcp => {
            let name = ws
                .right_expanded_id
                .as_deref()
                .and_then(|id| id.strip_prefix("mcp:"))
                .map(str::to_owned);
            name.map(|n| ws.stop_mcp(&n)).unwrap_or(false)
        }
        ClientAction::DismissReminder => {
            if ws.reminder.is_none() {
                false
            } else {
                ws.dismiss_reminder();
                true
            }
        }
        ClientAction::DeleteThread => ws.delete_thread(ws.selected),
        ClientAction::CycleModel => {
            let before = ws.model.clone();
            ws.cycle_model();
            ws.model != before
        }
        ClientAction::TogglePalette => {
            ws.toggle_overlay(crate::overlay::OverlayKind::Palette);
            true
        }
        ClientAction::ClosePalette => {
            if !ws.palette_open {
                false
            } else {
                ws.close_overlay(crate::overlay::OverlayKind::Palette);
                true
            }
        }
        ClientAction::ToggleHelp => {
            ws.toggle_overlay(crate::overlay::OverlayKind::Help);
            true
        }
        ClientAction::Send
        | ClientAction::Interrupt
        | ClientAction::RefreshCores
        | ClientAction::RefreshMcp
        | ClientAction::CreateCheckpoint
        | ClientAction::Approve
        | ClientAction::Deny
        | ClientAction::RestoreCheckpoint
        | ClientAction::RunTerminal
        | ClientAction::RefreshGit
        | ClientAction::CycleFile
        | ClientAction::CopyLastMessage
        | ClientAction::LaunchGrokTui
        | ClientAction::StopGrokTui
        | ClientAction::OpenBrowser
        | ClientAction::CopySession
        | ClientAction::ReloadDiffs
        | ClientAction::RunGitStatus => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Workspace {
        Workspace::new("p", "m")
    }

    fn host_noops() -> [ClientAction; 18] {
        [
            ClientAction::Send,
            ClientAction::Interrupt,
            ClientAction::RefreshCores,
            ClientAction::RefreshMcp,
            ClientAction::CreateCheckpoint,
            ClientAction::Approve,
            ClientAction::Deny,
            ClientAction::RestoreCheckpoint,
            ClientAction::RunTerminal,
            ClientAction::RefreshGit,
            ClientAction::CycleFile,
            ClientAction::CopyLastMessage,
            ClientAction::LaunchGrokTui,
            ClientAction::StopGrokTui,
            ClientAction::OpenBrowser,
            ClientAction::CopySession,
            ClientAction::ReloadDiffs,
            ClientAction::RunGitStatus,
        ]
    }

    #[test]
    fn new_thread_appends_and_selects() {
        let mut ws = fresh();
        ws.set_draft("keep me?");
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.selected, 0);
        assert!(apply_layout_action(&mut ws, ClientAction::NewThread));
        assert_eq!(ws.threads.len(), 2);
        assert_eq!(ws.selected, 1);
        assert_eq!(ws.threads[1].id, "thr-2");
        assert!(ws.draft.is_empty());
        assert!(apply_layout_action(&mut ws, ClientAction::NewThread));
        assert_eq!(ws.threads.len(), 3);
        assert_eq!(ws.selected, 2);
    }

    #[test]
    fn select_thread_changes_only_when_index_moves() {
        let mut ws = fresh();
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectThread(0)));
        assert_eq!(ws.selected, 0);
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectThread(9)));
        assert_eq!(ws.selected, 0);
        let n = ws.threads.len();
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectThread(n)));
        assert_eq!(ws.selected, 0);

        assert!(apply_layout_action(&mut ws, ClientAction::NewThread));
        assert_eq!(ws.selected, 1);
        assert!(apply_layout_action(&mut ws, ClientAction::SelectThread(0)));
        assert_eq!(ws.selected, 0);
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectThread(0)));
        assert_eq!(ws.selected, 0);
        assert!(apply_layout_action(&mut ws, ClientAction::SelectThread(1)));
        assert_eq!(ws.selected, 1);
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectThread(4)));
        assert_eq!(ws.selected, 1);
    }

    #[test]
    fn toggle_left_and_right_flip_only_that_rail() {
        let mut ws = fresh();
        assert!(ws.chrome.left_open() && ws.chrome.right_open());

        assert!(apply_layout_action(&mut ws, ClientAction::ToggleLeft));
        assert!(!ws.chrome.left_open());
        assert!(ws.chrome.right_open());

        assert!(apply_layout_action(&mut ws, ClientAction::ToggleRight));
        assert!(!ws.chrome.left_open());
        assert!(!ws.chrome.right_open());

        assert!(apply_layout_action(&mut ws, ClientAction::ToggleLeft));
        assert!(ws.chrome.left_open());
        assert!(!ws.chrome.right_open());

        assert!(apply_layout_action(&mut ws, ClientAction::ToggleRight));
        assert!(ws.chrome.left_open() && ws.chrome.right_open());
    }

    #[test]
    fn select_tab_reopens_hidden_right_rail() {
        let mut ws = fresh();
        ws.chrome.hide_right();
        assert!(!ws.chrome.right_open());
        assert!(apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Session)
        ));
        assert!(ws.chrome.right_open());
        assert_eq!(ws.inspector, InspectorTab::Session);
    }

    #[test]
    fn select_tab_changes_only_when_different() {
        let mut ws = fresh();
        assert_eq!(ws.inspector, InspectorTab::Session);
        assert!(!apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Session)
        ));
        assert_eq!(ws.inspector, InspectorTab::Session);

        assert!(apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Resources)
        ));
        assert_eq!(ws.inspector, InspectorTab::Resources);
        assert!(!apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Resources)
        ));

        assert!(apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Mcp)
        ));
        assert_eq!(ws.inspector, InspectorTab::Mcp);

        assert!(apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Checkpoints)
        ));
        assert_eq!(ws.inspector, InspectorTab::Checkpoints);
        assert!(!apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Checkpoints)
        ));

        assert!(apply_layout_action(
            &mut ws,
            ClientAction::SelectTab(InspectorTab::Session)
        ));
        assert_eq!(ws.inspector, InspectorTab::Session);
    }

    #[test]
    fn dismiss_reminder_clears_only_when_present() {
        let mut ws = fresh();
        assert!(ws.reminder.is_none());
        assert!(!apply_layout_action(&mut ws, ClientAction::DismissReminder));
        assert!(ws.reminder.is_none());

        ws.set_reminder("main", "C:/repo");
        assert!(ws.reminder.is_some());
        assert!(apply_layout_action(&mut ws, ClientAction::DismissReminder));
        assert!(ws.reminder.is_none());
        assert!(!apply_layout_action(&mut ws, ClientAction::DismissReminder));
        assert!(ws.reminder.is_none());
    }

    #[test]
    fn delete_thread_via_action() {
        let mut ws = fresh();
        let only = ws.threads[0].id.clone();
        assert_eq!(ws.threads.len(), 1);
        assert!(!apply_layout_action(&mut ws, ClientAction::DeleteThread));
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.selected, 0);
        assert_eq!(ws.threads[0].id, only);

        assert!(apply_layout_action(&mut ws, ClientAction::NewThread));
        assert!(apply_layout_action(&mut ws, ClientAction::NewThread));
        assert_eq!(ws.threads.len(), 3);
        assert_eq!(ws.selected, 2);
        let keep0 = ws.threads[0].id.clone();
        let keep1 = ws.threads[1].id.clone();
        let drop_last = ws.threads[2].id.clone();
        assert!(apply_layout_action(&mut ws, ClientAction::DeleteThread));
        assert_eq!(ws.threads.len(), 2);
        assert_eq!(ws.selected, 1);
        assert_eq!(ws.threads[0].id, keep0);
        assert_eq!(ws.threads[1].id, keep1);
        assert!(ws.threads.iter().all(|t| t.id != drop_last));

        assert!(apply_layout_action(&mut ws, ClientAction::SelectThread(0)));
        assert_eq!(ws.selected, 0);
        assert!(apply_layout_action(&mut ws, ClientAction::NewThread));
        assert_eq!(ws.threads.len(), 3);
        assert!(apply_layout_action(&mut ws, ClientAction::SelectThread(1)));
        let after_mid0 = ws.threads[0].id.clone();
        let drop_mid = ws.threads[1].id.clone();
        let after_mid2 = ws.threads[2].id.clone();
        assert!(apply_layout_action(&mut ws, ClientAction::DeleteThread));
        assert_eq!(ws.threads.len(), 2);
        assert_eq!(ws.selected, 0);
        assert_eq!(ws.threads[0].id, after_mid0);
        assert_eq!(ws.threads[1].id, after_mid2);
        assert!(ws.threads.iter().all(|t| t.id != drop_mid));

        let mut first = fresh();
        assert!(apply_layout_action(&mut first, ClientAction::NewThread));
        assert!(apply_layout_action(&mut first, ClientAction::NewThread));
        assert!(apply_layout_action(
            &mut first,
            ClientAction::SelectThread(0)
        ));
        let remain1 = first.threads[1].id.clone();
        let remain2 = first.threads[2].id.clone();
        let drop_first = first.threads[0].id.clone();
        assert!(apply_layout_action(&mut first, ClientAction::DeleteThread));
        assert_eq!(first.threads.len(), 2);
        assert_eq!(first.selected, 0);
        assert_eq!(first.threads[0].id, remain1);
        assert_eq!(first.threads[1].id, remain2);
        assert!(first.threads.iter().all(|t| t.id != drop_first));

        assert!(apply_layout_action(&mut ws, ClientAction::DeleteThread));
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.selected, 0);
        let last = ws.threads[0].id.clone();
        assert!(!apply_layout_action(&mut ws, ClientAction::DeleteThread));
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.selected, 0);
        assert_eq!(ws.threads[0].id, last);

        ws.threads.clear();
        ws.selected = 0;
        assert!(!apply_layout_action(&mut ws, ClientAction::DeleteThread));
        assert!(ws.threads.is_empty());
        assert_eq!(ws.selected, 0);

        let mut oob = fresh();
        assert!(apply_layout_action(&mut oob, ClientAction::NewThread));
        assert_eq!(oob.threads.len(), 2);
        oob.selected = oob.threads.len();
        let before = oob.clone();
        assert!(!apply_layout_action(&mut oob, ClientAction::DeleteThread));
        assert_eq!(oob, before);
    }

    #[test]
    fn palette_help_actions() {
        let mut ws = fresh();
        assert!(!ws.palette_open && !ws.help_open);
        assert!(apply_layout_action(&mut ws, ClientAction::TogglePalette));
        assert!(ws.palette_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ClosePalette));
        assert!(!ws.palette_open);
        assert!(!apply_layout_action(&mut ws, ClientAction::ClosePalette));
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleHelp));
        assert!(ws.help_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleHelp));
        assert!(!ws.help_open);
    }

    #[test]
    fn cycle_model_action_rotates_when_catalog_has_two() {
        let mut ws = fresh();
        assert!(!apply_layout_action(&mut ws, ClientAction::CycleModel));
        ws.set_models(vec!["grok".into(), "fake".into()]);
        ws.model = "grok".into();
        assert!(apply_layout_action(&mut ws, ClientAction::CycleModel));
        assert_eq!(ws.model, "fake");
    }

    #[test]
    fn host_actions_include_new_noops() {
        let mut ws = fresh();
        ws.set_draft("hello");
        ws.set_reminder("main", "C:/repo");
        ws.busy = true;
        let snapshot = ws.clone();
        for action in [
            ClientAction::Approve,
            ClientAction::Deny,
            ClientAction::RestoreCheckpoint,
            ClientAction::RunTerminal,
            ClientAction::RefreshGit,
            ClientAction::CycleFile,
            ClientAction::CopyLastMessage,
        ] {
            let mut copy = snapshot.clone();
            assert!(
                !apply_layout_action(&mut copy, action),
                "{action:?} must stay a host no-op"
            );
            assert_eq!(copy, snapshot, "{action:?} must not mutate workspace");
        }
    }

    #[test]
    fn center_mode_and_diff_sort_actions() {
        let mut ws = fresh();
        assert_eq!(ws.center_mode, crate::center::CenterMode::Gui);
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleCenterMode));
        assert_eq!(ws.center_mode, crate::center::CenterMode::GrokTui);
        assert!(!apply_layout_action(&mut ws, ClientAction::SetCenterTui));
        assert!(apply_layout_action(&mut ws, ClientAction::SetCenterGui));
        assert_eq!(ws.center_mode, crate::center::CenterMode::Gui);
        assert!(apply_layout_action(&mut ws, ClientAction::SortDiffFileName));
        assert_eq!(ws.diff_sort, crate::diff_view::DiffSort::FileName);
        assert!(!apply_layout_action(
            &mut ws,
            ClientAction::SortDiffFileName
        ));
        assert!(apply_layout_action(&mut ws, ClientAction::SortDiffLastTurn));
        assert!(!apply_layout_action(&mut ws, ClientAction::LaunchGrokTui));
        assert!(!apply_layout_action(&mut ws, ClientAction::OpenBrowser));
    }

    #[test]
    fn start_stop_mcp_follow_expanded_row() {
        let mut ws = fresh();
        ws.mcp.push(crate::workspace::McpRow {
            name: "linear".into(),
            command: "npx".into(),
            transport: "stdio".into(),
            state: crate::workspace::McpLife::Stopped,
        });
        assert!(!apply_layout_action(&mut ws, ClientAction::StartMcp));
        ws.toggle_right_row("mcp:linear");
        assert!(apply_layout_action(&mut ws, ClientAction::StartMcp));
        assert_eq!(ws.mcp[0].state, crate::workspace::McpLife::Ready);
        assert!(apply_layout_action(&mut ws, ClientAction::StopMcp));
        assert_eq!(ws.mcp[0].state, crate::workspace::McpLife::Stopped);
        ws.toggle_right_row("mcp:linear");
        assert!(!apply_layout_action(&mut ws, ClientAction::StartMcp));
    }

    #[test]
    fn select_model_applies_settings_default() {
        let mut ws = fresh();
        ws.set_models(vec!["grok".into(), "fake".into()]);
        ws.settings.set_default_model("fake");
        assert_eq!(ws.model, "grok");
        assert!(apply_layout_action(&mut ws, ClientAction::SelectModel));
        assert_eq!(ws.model, "fake");
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectModel));
        ws.settings.set_default_model("missing");
        assert!(!apply_layout_action(&mut ws, ClientAction::SelectModel));
        assert_eq!(ws.model, "fake");
    }

    #[test]
    fn hide_left_is_hidden_and_focus_layout_works() {
        let mut ws = fresh();
        assert!(apply_layout_action(&mut ws, ClientAction::HideLeft));
        assert_eq!(ws.chrome.left, crate::workspace::RailVis::Hidden);
        assert_eq!(ws.chrome.occupied_left(), 0.0);
        assert!(apply_layout_action(&mut ws, ClientAction::FocusLayout));
        assert!(ws.is_focus_layout());
        assert!(apply_layout_action(&mut ws, ClientAction::FocusLayout));
        assert!(!ws.is_focus_layout());
    }

    #[test]
    fn settings_and_file_mention_actions() {
        let mut ws = fresh();
        assert!(!ws.settings_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleSettings));
        assert!(ws.settings_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleSettings));
        assert!(!ws.settings_open);
        assert!(apply_layout_action(&mut ws, ClientAction::TogglePalette));
        assert!(ws.palette_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleSettings));
        assert!(ws.settings_open && !ws.palette_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ToggleSearch));
        assert!(ws.search_open && !ws.settings_open);
        assert!(apply_layout_action(&mut ws, ClientAction::CloseOverlay));
        assert!(!ws.search_open);
        assert!(apply_layout_action(&mut ws, ClientAction::ResetOutlook));
        assert!(ws.chrome.left_open() && ws.chrome.right_open());
        ws.set_files(vec!["src/lib.rs".into()]);
        assert!(ws.select_file("src/lib.rs"));
        assert!(apply_layout_action(
            &mut ws,
            ClientAction::InsertFileMention
        ));
        assert!(ws.draft.contains("`@src/lib.rs`"));
        assert!(!apply_layout_action(&mut ws, ClientAction::CreateWorktree));
    }

    #[test]
    fn host_actions_do_not_mutate_workspace() {
        let mut ws = fresh();
        ws.set_draft("hello");
        ws.set_reminder("main", "C:/repo");
        ws.busy = true;
        let snapshot = ws.clone();
        for action in host_noops() {
            let mut copy = snapshot.clone();
            assert!(
                !apply_layout_action(&mut copy, action),
                "{action:?} must stay a host no-op"
            );
            assert_eq!(copy, snapshot, "{action:?} must not mutate workspace");
        }
    }
}
