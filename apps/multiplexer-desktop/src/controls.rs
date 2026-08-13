//! Headless catalog of every window control the GPUI shell must project.
//!
//! No GPUI types. Parent matches [`ControlSpec::action`] in `ShellView`.
//! Every visible (or soon-visible) control has a handler name. Nothing is dead.

/// Window region a control is projected into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Surface {
    TitleBar,
    LeftRail,
    Center,
    RightRail,
    Composer,
    TermStrip,
    Palette,
    HelpOverlay,
    ApprovalCard,
    ReminderBar,
    Search,
    Settings,
}

impl Surface {
    /// Every surface the window must paint. Order is the Outlook layout.
    pub const fn all() -> [Surface; 12] {
        [
            Self::TitleBar,
            Self::LeftRail,
            Self::Center,
            Self::RightRail,
            Self::Composer,
            Self::TermStrip,
            Self::Palette,
            Self::HelpOverlay,
            Self::ApprovalCard,
            Self::ReminderBar,
            Self::Search,
            Self::Settings,
        ]
    }
}

/// One clickable or key-bound control. `action` is the stable ShellView arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlSpec {
    pub id: &'static str,
    pub surface: Surface,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub action: &'static str,
}

impl ControlSpec {
    /// True when id, label, and action are all non-empty live text.
    pub fn is_live(self) -> bool {
        is_live_text(self.id) && is_live_text(self.label) && is_live_text(self.action)
    }
}

/// Ids the parent rewrite must implement. Order matches the product checklist.
pub const REQUIRED_IDS: &[&str] = &[
    "chats_toggle",
    "inspector_toggle",
    "stop",
    "command_palette",
    "help",
    "run",
    "layout_reset",
    "settings",
    "center_gui",
    "center_tui",
    "launch_tui",
    "new_thread",
    "select_thread",
    "delete_thread",
    "chip_what",
    "chip_summarize",
    "chip_git_status",
    "chip_test",
    "copy_last_message",
    "send",
    "newline",
    "paste",
    "tab_session",
    "tab_cores",
    "tab_mcp",
    "tab_points",
    "tab_git",
    "tab_term",
    "tab_skills",
    "tab_diff",
    "tab_browser",
    "sort_last_turn",
    "sort_file_name",
    "open_browser",
    "cycle_model",
    "copy_session",
    "refresh_cores",
    "refresh_mcp",
    "create_checkpoint",
    "revert_checkpoint",
    "refresh_git",
    "run_git_status",
    "term_run",
    "term_clear",
    "toggle_bottom",
    "palette_filter",
    "palette_run",
    "help_close",
    "allow",
    "deny",
    "dismiss",
    "project_pill",
    "branch_pill",
    "turns_pill",
    "remotes_pill",
    "focus_layout",
    "hide_left",
    "hide_right",
    "hide_bottom",
    "left_section_threads",
    "left_section_agents",
    "left_section_files",
    "left_section_activity",
    "tab_files",
    "tab_activity",
    "tab_agents",
    "stop_tui",
    "toggle_search",
    "settings_close",
    "settings_theme",
    "settings_density",
    "toast_dismiss",
    "refresh_files",
    "reveal_file",
    "open_external",
];

const fn spec(
    id: &'static str,
    surface: Surface,
    label: &'static str,
    shortcut: Option<&'static str>,
) -> ControlSpec {
    ControlSpec {
        id,
        surface,
        label,
        shortcut,
        action: id,
    }
}

const CONTROLS: &[ControlSpec] = &[
    spec("chats_toggle", Surface::TitleBar, "Chats", Some("ctrl-[")),
    spec(
        "inspector_toggle",
        Surface::TitleBar,
        "Inspector",
        Some("ctrl-]"),
    ),
    spec("stop", Surface::TitleBar, "Stop", Some("ctrl-.")),
    spec(
        "command_palette",
        Surface::TitleBar,
        "Command palette",
        Some("ctrl-k"),
    ),
    spec("help", Surface::TitleBar, "Help", Some("f1")),
    spec("run", Surface::TitleBar, "Run", None),
    spec("layout_reset", Surface::TitleBar, "Reset layout", None),
    spec("settings", Surface::TitleBar, "Settings", Some("f2")),
    spec("center_gui", Surface::Center, "Chat log", None),
    spec(
        "center_tui",
        Surface::Center,
        "Grok TUI",
        Some("ctrl-shift-g"),
    ),
    spec("launch_tui", Surface::Center, "Launch Grok TUI", None),
    spec("new_thread", Surface::LeftRail, "New", Some("ctrl-n")),
    spec("select_thread", Surface::LeftRail, "Select thread", None),
    spec("delete_thread", Surface::LeftRail, "Delete thread", None),
    spec("chip_what", Surface::Center, "What can you do?", None),
    spec(
        "chip_summarize",
        Surface::Center,
        "Summarize this repo",
        None,
    ),
    spec("chip_git_status", Surface::Center, "Git status", None),
    spec("chip_test", Surface::Center, "Run the tests", None),
    spec(
        "copy_last_message",
        Surface::Center,
        "Copy last message",
        None,
    ),
    spec("send", Surface::Composer, "Send", Some("enter")),
    spec("newline", Surface::Composer, "Newline", Some("shift-enter")),
    spec("paste", Surface::Composer, "Paste", Some("ctrl-v")),
    spec("tab_session", Surface::RightRail, "Session", None),
    spec("tab_cores", Surface::RightRail, "Cores", None),
    spec("tab_mcp", Surface::RightRail, "MCP", None),
    spec("tab_points", Surface::RightRail, "Points", None),
    spec("tab_git", Surface::RightRail, "Git", None),
    spec("tab_term", Surface::RightRail, "Term", None),
    spec("tab_skills", Surface::RightRail, "Skills", None),
    spec("tab_diff", Surface::RightRail, "Diffs", None),
    spec("tab_browser", Surface::RightRail, "Browser", None),
    spec("sort_last_turn", Surface::RightRail, "Last turn", None),
    spec("sort_file_name", Surface::RightRail, "File name", None),
    spec("open_browser", Surface::RightRail, "Open browser", None),
    spec("cycle_model", Surface::RightRail, "Cycle model", None),
    spec("copy_session", Surface::RightRail, "Copy session", None),
    spec("refresh_cores", Surface::RightRail, "Refresh cores", None),
    spec("refresh_mcp", Surface::RightRail, "Refresh MCP", None),
    spec(
        "create_checkpoint",
        Surface::RightRail,
        "Create checkpoint",
        None,
    ),
    spec(
        "revert_checkpoint",
        Surface::RightRail,
        "Revert checkpoint",
        None,
    ),
    spec("refresh_git", Surface::RightRail, "Refresh git", None),
    spec("run_git_status", Surface::RightRail, "Run git status", None),
    spec("term_run", Surface::TermStrip, "Run", None),
    spec("term_clear", Surface::TermStrip, "Clear", None),
    spec(
        "toggle_bottom",
        Surface::TermStrip,
        "Toggle terminal",
        Some("ctrl-`"),
    ),
    spec("palette_filter", Surface::Palette, "Filter", None),
    spec("palette_run", Surface::Palette, "Run command", None),
    spec("help_close", Surface::HelpOverlay, "Close", Some("escape")),
    spec("allow", Surface::ApprovalCard, "Allow", Some("a")),
    spec("deny", Surface::ApprovalCard, "Deny", Some("d")),
    spec("dismiss", Surface::ReminderBar, "Dismiss", Some("escape")),
    spec("project_pill", Surface::TitleBar, "Project", None),
    spec("branch_pill", Surface::TitleBar, "Branch", None),
    spec("turns_pill", Surface::TitleBar, "Turns", None),
    spec("remotes_pill", Surface::TitleBar, "Remotes", None),
    spec(
        "focus_layout",
        Surface::TitleBar,
        "Focus layout",
        Some("ctrl-shift-h"),
    ),
    spec("hide_left", Surface::LeftRail, "Hide left", None),
    spec("hide_right", Surface::RightRail, "Hide right", None),
    spec("hide_bottom", Surface::TermStrip, "Hide terminal", None),
    spec(
        "left_section_threads",
        Surface::LeftRail,
        "Chats",
        Some("ctrl-1"),
    ),
    spec(
        "left_section_agents",
        Surface::LeftRail,
        "Agents",
        Some("ctrl-2"),
    ),
    spec(
        "left_section_files",
        Surface::LeftRail,
        "Projects",
        Some("ctrl-3"),
    ),
    spec(
        "left_section_activity",
        Surface::LeftRail,
        "Activity",
        Some("ctrl-4"),
    ),
    spec("tab_files", Surface::RightRail, "Files", None),
    spec("tab_activity", Surface::RightRail, "Activity", None),
    spec("tab_agents", Surface::RightRail, "Agents", None),
    spec("stop_tui", Surface::Center, "Stop Grok TUI", None),
    spec(
        "toggle_search",
        Surface::Search,
        "Search names",
        Some("ctrl-p"),
    ),
    spec(
        "settings_close",
        Surface::Settings,
        "Close settings",
        Some("escape"),
    ),
    spec("settings_theme", Surface::Settings, "Theme", None),
    spec("settings_density", Surface::Settings, "Density", None),
    spec("toast_dismiss", Surface::TitleBar, "Dismiss toast", None),
    spec("refresh_files", Surface::RightRail, "Reload files", None),
    spec("reveal_file", Surface::RightRail, "Reveal file", None),
    spec("open_external", Surface::RightRail, "Open external", None),
];

/// Global chords. Escape is `close_overlay` (palette, help, search, settings).
const SHORTCUTS: &[(&str, &str)] = &[
    ("enter", "send"),
    ("escape", "close_overlay"),
    ("ctrl-k", "command_palette"),
    ("ctrl-shift-p", "command_palette"),
    ("ctrl-n", "new_thread"),
    ("ctrl-[", "chats_toggle"),
    ("ctrl-]", "inspector_toggle"),
    ("ctrl-.", "stop"),
    ("ctrl-p", "toggle_search"),
    ("ctrl-shift-f", "toggle_search"),
    ("ctrl-v", "paste"),
    ("shift-enter", "newline"),
    ("f1", "help"),
    ("ctrl-`", "toggle_bottom"),
    ("f2", "settings"),
    ("ctrl-,", "settings"),
    ("ctrl-shift-g", "center_tui"),
    ("ctrl-shift-l", "layout_reset"),
    ("ctrl-shift-h", "focus_layout"),
    ("ctrl-1", "left_section_threads"),
    ("ctrl-2", "left_section_agents"),
    ("ctrl-3", "left_section_files"),
    ("ctrl-4", "left_section_activity"),
];

fn is_live_text(s: &str) -> bool {
    !s.is_empty() && s.chars().any(|c| !c.is_whitespace())
}

/// Full window checklist, Outlook order.
pub fn all_controls() -> Vec<ControlSpec> {
    CONTROLS.to_vec()
}

/// Lookup by stable id. Exact match, case sensitive.
pub fn control_by_id(id: &str) -> Option<ControlSpec> {
    CONTROLS.iter().copied().find(|c| c.id == id)
}

/// Controls painted on `surface`, catalog order.
pub fn controls_on(surface: Surface) -> Vec<ControlSpec> {
    CONTROLS
        .iter()
        .copied()
        .filter(|c| c.surface == surface)
        .collect()
}

/// True when every control has a non-empty id, label, and action.
pub fn no_dead_labels() -> bool {
    let controls = all_controls();
    !controls.is_empty() && controls.iter().all(|c| c.is_live())
}

/// Chord to action. Keys are unique. `ctrl-k` and `ctrl-p` both open the palette.
pub fn shortcut_map() -> Vec<(&'static str, &'static str)> {
    SHORTCUTS.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> Vec<&'static str> {
        all_controls().into_iter().map(|c| c.id).collect()
    }

    fn shortcut_action(key: &str) -> Option<&'static str> {
        shortcut_map()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, a)| a)
    }

    #[test]
    fn all_required_ids_present() {
        let have = ids();
        for required in REQUIRED_IDS {
            assert!(
                have.contains(required),
                "missing required control id {required}"
            );
            let spec = control_by_id(required).expect(required);
            assert_eq!(spec.id, *required);
            assert_eq!(spec.action, *required);
            assert!(spec.is_live(), "{required} is dead");
        }
        assert_eq!(REQUIRED_IDS.len(), 75);
        assert_eq!(have.len(), REQUIRED_IDS.len());
        assert_eq!(all_controls().len(), 75);

        let required_once = [
            "chats_toggle",
            "inspector_toggle",
            "stop",
            "command_palette",
            "help",
            "run",
            "layout_reset",
            "settings",
            "center_gui",
            "center_tui",
            "launch_tui",
            "new_thread",
            "select_thread",
            "delete_thread",
            "chip_what",
            "chip_summarize",
            "chip_git_status",
            "chip_test",
            "send",
            "newline",
            "paste",
            "tab_session",
            "tab_cores",
            "tab_mcp",
            "tab_points",
            "tab_git",
            "tab_term",
            "tab_skills",
            "tab_diff",
            "tab_browser",
            "sort_last_turn",
            "sort_file_name",
            "open_browser",
            "cycle_model",
            "copy_session",
            "refresh_cores",
            "refresh_mcp",
            "create_checkpoint",
            "revert_checkpoint",
            "refresh_git",
            "run_git_status",
            "term_run",
            "term_clear",
            "toggle_bottom",
            "palette_filter",
            "palette_run",
            "help_close",
            "allow",
            "deny",
            "dismiss",
            "copy_last_message",
        ];
        for id in required_once {
            assert!(have.contains(&id), "checklist id missing: {id}");
        }
        assert_eq!(required_once.len(), 51);
    }

    #[test]
    fn required_ids_live_on_their_surfaces() {
        let pin = [
            ("chats_toggle", Surface::TitleBar),
            ("inspector_toggle", Surface::TitleBar),
            ("stop", Surface::TitleBar),
            ("command_palette", Surface::TitleBar),
            ("help", Surface::TitleBar),
            ("run", Surface::TitleBar),
            ("layout_reset", Surface::TitleBar),
            ("settings", Surface::TitleBar),
            ("center_gui", Surface::Center),
            ("center_tui", Surface::Center),
            ("launch_tui", Surface::Center),
            ("new_thread", Surface::LeftRail),
            ("select_thread", Surface::LeftRail),
            ("delete_thread", Surface::LeftRail),
            ("chip_what", Surface::Center),
            ("chip_summarize", Surface::Center),
            ("chip_git_status", Surface::Center),
            ("chip_test", Surface::Center),
            ("copy_last_message", Surface::Center),
            ("send", Surface::Composer),
            ("newline", Surface::Composer),
            ("paste", Surface::Composer),
            ("tab_session", Surface::RightRail),
            ("tab_cores", Surface::RightRail),
            ("tab_mcp", Surface::RightRail),
            ("tab_points", Surface::RightRail),
            ("tab_git", Surface::RightRail),
            ("tab_term", Surface::RightRail),
            ("tab_skills", Surface::RightRail),
            ("tab_diff", Surface::RightRail),
            ("tab_browser", Surface::RightRail),
            ("sort_last_turn", Surface::RightRail),
            ("sort_file_name", Surface::RightRail),
            ("open_browser", Surface::RightRail),
            ("cycle_model", Surface::RightRail),
            ("copy_session", Surface::RightRail),
            ("refresh_cores", Surface::RightRail),
            ("refresh_mcp", Surface::RightRail),
            ("create_checkpoint", Surface::RightRail),
            ("revert_checkpoint", Surface::RightRail),
            ("refresh_git", Surface::RightRail),
            ("run_git_status", Surface::RightRail),
            ("term_run", Surface::TermStrip),
            ("term_clear", Surface::TermStrip),
            ("toggle_bottom", Surface::TermStrip),
            ("palette_filter", Surface::Palette),
            ("palette_run", Surface::Palette),
            ("help_close", Surface::HelpOverlay),
            ("allow", Surface::ApprovalCard),
            ("deny", Surface::ApprovalCard),
            ("dismiss", Surface::ReminderBar),
            ("project_pill", Surface::TitleBar),
            ("branch_pill", Surface::TitleBar),
            ("turns_pill", Surface::TitleBar),
            ("remotes_pill", Surface::TitleBar),
            ("focus_layout", Surface::TitleBar),
            ("hide_left", Surface::LeftRail),
            ("hide_right", Surface::RightRail),
            ("hide_bottom", Surface::TermStrip),
            ("left_section_threads", Surface::LeftRail),
            ("left_section_agents", Surface::LeftRail),
            ("left_section_files", Surface::LeftRail),
            ("left_section_activity", Surface::LeftRail),
            ("tab_files", Surface::RightRail),
            ("tab_activity", Surface::RightRail),
            ("tab_agents", Surface::RightRail),
            ("stop_tui", Surface::Center),
            ("toggle_search", Surface::Search),
            ("settings_close", Surface::Settings),
            ("settings_theme", Surface::Settings),
            ("settings_density", Surface::Settings),
            ("toast_dismiss", Surface::TitleBar),
            ("refresh_files", Surface::RightRail),
            ("reveal_file", Surface::RightRail),
            ("open_external", Surface::RightRail),
        ];
        for (id, surface) in pin {
            let spec = control_by_id(id).unwrap_or_else(|| panic!("missing {id}"));
            assert_eq!(spec.surface, surface, "{id} surface");
            assert_eq!(spec.action, id, "{id} action must be the handler name");
        }
        assert_eq!(pin.len(), all_controls().len());
    }

    #[test]
    fn shortcuts_cover_palette() {
        assert_eq!(shortcut_action("ctrl-k"), Some("command_palette"));
        assert_eq!(shortcut_action("ctrl-shift-p"), Some("command_palette"));
        assert_eq!(shortcut_action("ctrl-p"), Some("toggle_search"));
        assert_ne!(shortcut_action("ctrl-k"), Some("palette_run"));
        assert_ne!(shortcut_action("ctrl-p"), Some("help"));
        assert_ne!(shortcut_action("ctrl-p"), shortcut_action("ctrl-shift-p"));

        let palette = control_by_id("command_palette").expect("command_palette");
        assert_eq!(palette.surface, Surface::TitleBar);
        assert_eq!(palette.action, "command_palette");
        assert_eq!(palette.shortcut, Some("ctrl-k"));
        assert_eq!(palette.label, "Command palette");

        let on_palette = controls_on(Surface::Palette);
        assert_eq!(on_palette.len(), 2);
        assert_eq!(on_palette[0].id, "palette_filter");
        assert_eq!(on_palette[0].action, "palette_filter");
        assert_eq!(on_palette[1].id, "palette_run");
        assert_eq!(on_palette[1].action, "palette_run");
        assert!(control_by_id("palette_filter").is_some());
        assert!(control_by_id("palette_run").is_some());
    }

    #[test]
    fn no_empty_actions() {
        assert!(super::no_dead_labels());
        for c in all_controls() {
            assert!(!c.action.is_empty(), "{} has an empty action", c.id);
            assert!(!c.id.is_empty(), "empty id");
            assert!(!c.label.is_empty(), "{} has an empty label", c.id);
            assert!(
                c.action.chars().any(|ch| !ch.is_whitespace()),
                "{} action is whitespace",
                c.id
            );
            if let Some(keys) = c.shortcut {
                assert!(!keys.is_empty(), "{} has an empty shortcut", c.id);
            }
        }
    }

    #[test]
    fn no_dead_labels() {
        assert!(super::no_dead_labels());
        assert!(all_controls().iter().all(|c| c.is_live()));
        assert!(!ControlSpec {
            id: "",
            surface: Surface::Center,
            label: "x",
            shortcut: None,
            action: "x",
        }
        .is_live());
        assert!(!ControlSpec {
            id: "x",
            surface: Surface::Center,
            label: "",
            shortcut: None,
            action: "x",
        }
        .is_live());
        assert!(!ControlSpec {
            id: "x",
            surface: Surface::Center,
            label: "x",
            shortcut: None,
            action: "",
        }
        .is_live());
        assert!(!ControlSpec {
            id: "   ",
            surface: Surface::Center,
            label: "x",
            shortcut: None,
            action: "x",
        }
        .is_live());
    }

    #[test]
    fn surfaces_nonempty() {
        assert_eq!(Surface::all().len(), 12);
        for surface in Surface::all() {
            let on = controls_on(surface);
            assert!(!on.is_empty(), "{surface:?} must have at least one control");
            assert!(on.iter().all(|c| c.surface == surface));
        }
        assert_eq!(controls_on(Surface::TitleBar).len(), 14);
        assert_eq!(controls_on(Surface::LeftRail).len(), 8);
        assert_eq!(controls_on(Surface::Center).len(), 9);
        assert_eq!(controls_on(Surface::Composer).len(), 3);
        assert_eq!(controls_on(Surface::RightRail).len(), 27);
        assert_eq!(controls_on(Surface::TermStrip).len(), 4);
        assert_eq!(controls_on(Surface::Palette).len(), 2);
        assert_eq!(controls_on(Surface::HelpOverlay).len(), 1);
        assert_eq!(controls_on(Surface::ApprovalCard).len(), 2);
        assert_eq!(controls_on(Surface::ReminderBar).len(), 1);
        assert_eq!(controls_on(Surface::Search).len(), 1);
        assert_eq!(controls_on(Surface::Settings).len(), 3);
    }

    #[test]
    fn surface_match_is_exhaustive() {
        let tag = |s: Surface| match s {
            Surface::TitleBar => 0,
            Surface::LeftRail => 1,
            Surface::Center => 2,
            Surface::RightRail => 3,
            Surface::Composer => 4,
            Surface::TermStrip => 5,
            Surface::Palette => 6,
            Surface::HelpOverlay => 7,
            Surface::ApprovalCard => 8,
            Surface::ReminderBar => 9,
            Surface::Search => 10,
            Surface::Settings => 11,
        };
        let tags: Vec<u8> = Surface::all().into_iter().map(tag).collect();
        assert_eq!(tags, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        assert_ne!(Surface::TitleBar, Surface::LeftRail);
        assert_ne!(Surface::Palette, Surface::HelpOverlay);
        assert_ne!(Surface::ApprovalCard, Surface::ReminderBar);
    }

    #[test]
    fn shortcut_map_has_required_bindings() {
        assert_eq!(shortcut_map().len(), 23);
        assert_eq!(shortcut_action("ctrl-shift-g"), Some("center_tui"));
        assert_eq!(shortcut_action("enter"), Some("send"));
        assert_eq!(shortcut_action("escape"), Some("close_overlay"));
        assert_eq!(shortcut_action("ctrl-k"), Some("command_palette"));
        assert_eq!(shortcut_action("ctrl-n"), Some("new_thread"));
        assert_eq!(shortcut_action("ctrl-["), Some("chats_toggle"));
        assert_eq!(shortcut_action("ctrl-]"), Some("inspector_toggle"));
        assert_eq!(shortcut_action("ctrl-."), Some("stop"));
        assert_eq!(shortcut_action("ctrl-p"), Some("toggle_search"));
        assert_eq!(shortcut_action("ctrl-shift-p"), Some("command_palette"));
        assert_eq!(shortcut_action("ctrl-shift-f"), Some("toggle_search"));
        assert_eq!(shortcut_action("ctrl-,"), Some("settings"));
        assert_eq!(shortcut_action("ctrl-v"), Some("paste"));
        assert_eq!(shortcut_action("shift-enter"), Some("newline"));
        assert_eq!(shortcut_action("f1"), Some("help"));
        assert_eq!(shortcut_action("ctrl-`"), Some("toggle_bottom"));
        assert_eq!(shortcut_action("f2"), Some("settings"));
        assert_eq!(shortcut_action("ctrl-shift-k"), None);
        assert_eq!(
            shortcut_action("enter"),
            Some(control_by_id("send").unwrap().action)
        );
    }

    #[test]
    fn shortcut_targets_are_live_handlers() {
        let actions: Vec<_> = all_controls().iter().map(|c| c.action).collect();
        let mut keys = Vec::new();
        for (key, action) in shortcut_map() {
            assert!(is_live_text(key), "dead shortcut key");
            assert!(is_live_text(action), "dead shortcut action {key}");
            assert!(!keys.contains(&key), "duplicate shortcut {key}");
            keys.push(key);
            if action == "close_overlay" {
                assert_eq!(key, "escape");
                assert!(control_by_id("close_overlay").is_none());
                continue;
            }
            assert!(
                actions.contains(&action),
                "{key} maps to {action}, which is not a control action"
            );
        }
        assert_eq!(keys.len(), 23);
    }

    #[test]
    fn control_by_id_known_and_unknown() {
        let send = control_by_id("send").expect("send");
        assert_eq!(
            send,
            ControlSpec {
                id: "send",
                surface: Surface::Composer,
                label: "Send",
                shortcut: Some("enter"),
                action: "send",
            }
        );
        assert!(control_by_id("").is_none());
        assert!(control_by_id("nope").is_none());
        assert!(control_by_id("SEND").is_none());
        assert!(control_by_id("send ").is_none());
        assert!(control_by_id("help_").is_none());
        let help = control_by_id("help").expect("help");
        assert_eq!(help.id, "help");
        assert_ne!(help.id, "help_close");
        assert_eq!(help.action, "help");
        assert_eq!(
            control_by_id("help_close").map(|c| c.surface),
            Some(Surface::HelpOverlay)
        );
    }

    #[test]
    fn control_ids_are_unique() {
        let mut seen = ids();
        let n = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), n);
    }

    #[test]
    fn visible_labels_match_current_chrome() {
        assert_eq!(control_by_id("chats_toggle").unwrap().label, "Chats");
        assert_eq!(
            control_by_id("inspector_toggle").unwrap().label,
            "Inspector"
        );
        assert_eq!(control_by_id("stop").unwrap().label, "Stop");
        assert_eq!(control_by_id("run").unwrap().label, "Run");
        assert_eq!(control_by_id("layout_reset").unwrap().label, "Reset layout");
        assert_eq!(control_by_id("settings").unwrap().label, "Settings");
        assert_eq!(control_by_id("new_thread").unwrap().label, "New");
        assert_eq!(
            control_by_id("chip_what").unwrap().label,
            "What can you do?"
        );
        assert_eq!(
            control_by_id("chip_summarize").unwrap().label,
            "Summarize this repo"
        );
        assert_eq!(
            control_by_id("chip_git_status").unwrap().label,
            "Git status"
        );
        assert_eq!(control_by_id("chip_test").unwrap().label, "Run the tests");
        assert_eq!(
            control_by_id("toggle_bottom").unwrap().label,
            "Toggle terminal"
        );
        assert_eq!(control_by_id("send").unwrap().label, "Send");
        assert_eq!(control_by_id("dismiss").unwrap().label, "Dismiss");
        assert_eq!(control_by_id("tab_session").unwrap().label, "Session");
        assert_eq!(control_by_id("tab_cores").unwrap().label, "Cores");
        assert_eq!(control_by_id("tab_mcp").unwrap().label, "MCP");
        assert_eq!(control_by_id("tab_points").unwrap().label, "Points");
        assert_eq!(control_by_id("tab_git").unwrap().label, "Git");
        assert_eq!(control_by_id("tab_term").unwrap().label, "Term");
        assert_eq!(control_by_id("tab_skills").unwrap().label, "Skills");
        assert_eq!(control_by_id("allow").unwrap().label, "Allow");
        assert_eq!(control_by_id("deny").unwrap().label, "Deny");
    }

    #[test]
    fn controls_on_does_not_leak_other_surfaces() {
        for c in controls_on(Surface::Composer) {
            assert_ne!(c.id, "chip_what");
            assert_ne!(c.surface, Surface::Center);
        }
        let composer_ids: Vec<_> = controls_on(Surface::Composer)
            .into_iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(composer_ids, vec!["send", "newline", "paste"]);
        assert!(controls_on(Surface::TitleBar)
            .iter()
            .all(|c| c.id != "dismiss"));
        assert_eq!(
            controls_on(Surface::ReminderBar)
                .into_iter()
                .map(|c| c.action)
                .collect::<Vec<_>>(),
            vec!["dismiss"]
        );
    }

    #[test]
    fn actions_are_snake_case_handler_names() {
        for c in all_controls() {
            assert!(
                c.action
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
                "action {} is not a snake_case handler",
                c.action
            );
            assert!(
                c.action.starts_with(|ch: char| ch.is_ascii_lowercase()),
                "action {} must start with a letter",
                c.action
            );
            assert_eq!(c.action, c.id);
        }
    }
}
