//! Binding table: chord lookup for `handle_key`.

use crate::workspace::{InspectorTab, LeftSection};
use crate::ClientAction;

/// One key plus modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: String,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Chord {
    pub fn new(key: impl Into<String>, ctrl: bool, shift: bool, alt: bool) -> Self {
        let mut c = Self {
            key: key.into(),
            ctrl,
            shift,
            alt,
        };
        c.normalize();
        c
    }

    pub fn normalize(&mut self) {
        let lower = self.key.to_ascii_lowercase();
        self.key = match lower.as_str() {
            "oem_3" => "`".to_owned(),
            other => other.to_owned(),
        };
    }

    /// Parse `ctrl-shift-p`, `f1`, `escape`, `ctrl-,`.
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim().to_ascii_lowercase();
        if spec.is_empty() {
            return None;
        }
        let mut ctrl = false;
        let mut shift = false;
        let mut alt = false;
        let mut key: Option<&str> = None;
        for part in spec.split('-') {
            match part {
                "" => return None,
                "ctrl" | "control" => ctrl = true,
                "shift" => shift = true,
                "alt" => alt = true,
                other => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(other);
                }
            }
        }
        let key = key?;
        Some(Self::new(key, ctrl, shift, alt))
    }

    pub fn encode(&self) -> String {
        let mut out = String::new();
        if self.ctrl {
            out.push_str("ctrl-");
        }
        if self.shift {
            out.push_str("shift-");
        }
        if self.alt {
            out.push_str("alt-");
        }
        out.push_str(&self.key);
        out
    }
}

/// Chord to [`ClientAction`]. Last bind for a chord wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingTable {
    rows: Vec<(Chord, ClientAction)>,
}

impl BindingTable {
    pub fn defaults() -> Self {
        let mut t = Self { rows: Vec::new() };
        for (spec, action) in DEFAULT_BINDINGS {
            if let Some(chord) = Chord::parse(spec) {
                t.bind(chord, *action);
            }
        }
        t
    }

    pub fn bind(&mut self, chord: Chord, action: ClientAction) {
        self.rows.retain(|(c, _)| c != &chord);
        self.rows.push((chord, action));
    }

    pub fn lookup(&self, chord: &Chord) -> Option<ClientAction> {
        let mut needle = chord.clone();
        needle.normalize();
        self.rows
            .iter()
            .rev()
            .find(|(c, _)| *c == needle)
            .map(|(_, a)| *a)
    }

    pub fn lookup_spec(&self, spec: &str) -> Option<ClientAction> {
        Chord::parse(spec).and_then(|c| self.lookup(&c))
    }

    pub fn pairs(&self) -> Vec<(String, String)> {
        self.rows
            .iter()
            .filter_map(|(c, a)| action_id(*a).map(|id| (c.encode(), id.to_owned())))
            .collect()
    }

    /// User pairs overlay defaults. Unknown ids or chords are skipped.
    pub fn from_pairs(pairs: &[(String, String)]) -> Self {
        let mut t = Self::defaults();
        for (spec, id) in pairs {
            if let (Some(chord), Some(action)) = (Chord::parse(spec), action_from_id(id)) {
                t.bind(chord, action);
            }
        }
        t
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Default for BindingTable {
    fn default() -> Self {
        Self::defaults()
    }
}

const DEFAULT_BINDINGS: &[(&str, ClientAction)] = &[
    ("ctrl-k", ClientAction::TogglePalette),
    ("ctrl-shift-p", ClientAction::TogglePalette),
    ("ctrl-p", ClientAction::ToggleSearch),
    ("ctrl-shift-f", ClientAction::ToggleSearch),
    ("ctrl-,", ClientAction::ToggleSettings),
    ("f2", ClientAction::ToggleSettings),
    ("f1", ClientAction::ToggleHelp),
    ("ctrl-n", ClientAction::NewThread),
    ("ctrl-[", ClientAction::ToggleLeft),
    ("ctrl-]", ClientAction::ToggleRight),
    ("ctrl-b", ClientAction::ToggleLeft),
    ("ctrl-`", ClientAction::ToggleBottom),
    ("ctrl-.", ClientAction::Interrupt),
    ("ctrl-s", ClientAction::CreateCheckpoint),
    ("ctrl-shift-g", ClientAction::ToggleCenterMode),
    ("ctrl-shift-l", ClientAction::ResetOutlook),
    ("ctrl-shift-h", ClientAction::FocusLayout),
    (
        "ctrl-1",
        ClientAction::SelectLeftSection(LeftSection::Threads),
    ),
    (
        "ctrl-2",
        ClientAction::SelectLeftSection(LeftSection::Agents),
    ),
    (
        "ctrl-3",
        ClientAction::SelectLeftSection(LeftSection::Files),
    ),
    (
        "ctrl-4",
        ClientAction::SelectLeftSection(LeftSection::Activity),
    ),
    ("escape", ClientAction::CloseOverlay),
    ("enter", ClientAction::Send),
    ("ctrl-shift-d", ClientAction::PopOutInspector),
    ("ctrl-shift-e", ClientAction::DockInspector),
    ("ctrl-w", ClientAction::ClosePopOut),
    ("ctrl-tab", ClientAction::NextRegion),
    ("ctrl-alt-up", ClientAction::NudgeBottomUp),
    ("ctrl-alt-down", ClientAction::NudgeBottomDown),
];

/// Stable catalog / settings id for a bindable action.
pub fn action_id(action: ClientAction) -> Option<&'static str> {
    Some(match action {
        ClientAction::TogglePalette => "command_palette",
        ClientAction::ToggleSearch => "toggle_search",
        ClientAction::ToggleSettings => "settings",
        ClientAction::ToggleHelp => "help",
        ClientAction::NewThread => "new_thread",
        ClientAction::ToggleLeft => "chats_toggle",
        ClientAction::ToggleRight => "inspector_toggle",
        ClientAction::ToggleBottom => "toggle_bottom",
        ClientAction::Interrupt => "stop",
        ClientAction::CreateCheckpoint => "create_checkpoint",
        ClientAction::ToggleCenterMode | ClientAction::SetCenterTui => "center_tui",
        ClientAction::ResetOutlook => "layout_reset",
        ClientAction::FocusLayout => "focus_layout",
        ClientAction::SelectLeftSection(LeftSection::Threads) => "left_section_threads",
        ClientAction::SelectLeftSection(LeftSection::Agents) => "left_section_agents",
        ClientAction::SelectLeftSection(LeftSection::Files) => "left_section_files",
        ClientAction::SelectLeftSection(LeftSection::Activity) => "left_section_activity",
        ClientAction::CloseOverlay => "close_overlay",
        ClientAction::Send => "send",
        ClientAction::CopyLastMessage => "copy_last_message",
        ClientAction::ClosePalette => "command_palette",
        ClientAction::CloseSearch => "toggle_search",
        ClientAction::HideLeft => "hide_left",
        ClientAction::HideRight => "hide_right",
        ClientAction::HideBottom => "hide_bottom",
        ClientAction::SelectTab(InspectorTab::Session) => "tab_session",
        ClientAction::SelectTab(InspectorTab::Resources) => "tab_cores",
        ClientAction::SelectTab(InspectorTab::Mcp) => "tab_mcp",
        ClientAction::SelectTab(InspectorTab::Checkpoints) => "tab_points",
        ClientAction::SelectTab(InspectorTab::Git) => "tab_git",
        ClientAction::SelectTab(InspectorTab::Terminal) => "tab_term",
        ClientAction::SelectTab(InspectorTab::Skills) => "tab_skills",
        ClientAction::SelectTab(InspectorTab::Diff) => "tab_diff",
        ClientAction::SelectTab(InspectorTab::Browser) => "tab_browser",
        ClientAction::SelectTab(InspectorTab::Files) => "tab_files",
        ClientAction::SelectTab(InspectorTab::Activity) => "tab_activity",
        ClientAction::SelectTab(InspectorTab::Agents) => "tab_agents",
        ClientAction::DismissToast => "toast_dismiss",
        ClientAction::CopySession => "copy_session",
        ClientAction::RunGitStatus => "run_git_status",
        ClientAction::PopOutInspector => "popout_inspector",
        ClientAction::DockInspector => "dock_inspector",
        ClientAction::ClosePopOut => "close_popout",
        ClientAction::NextRegion => "next_region",
        ClientAction::NudgeBottomUp => "nudge_bottom_up",
        ClientAction::NudgeBottomDown => "nudge_bottom_down",
        ClientAction::ApproveOnce => "allow_once",
        ClientAction::Later => "later",
        ClientAction::PinThread => "pin_thread",
        ClientAction::MarkUnread => "mark_unread",
        ClientAction::ArchiveThread => "archive_thread",
        ClientAction::UnarchiveThread => "unarchive_thread",
        ClientAction::CopyThreadId => "copy_thread_id",
        ClientAction::OpenAbout => "settings_about",
        _ => return None,
    })
}

/// Inverse of [`action_id`] for bindable ids. Aliases accepted.
pub fn action_from_id(id: &str) -> Option<ClientAction> {
    Some(match id {
        "command_palette" | "toggle_palette" => ClientAction::TogglePalette,
        "toggle_search" => ClientAction::ToggleSearch,
        "settings" => ClientAction::ToggleSettings,
        "help" => ClientAction::ToggleHelp,
        "new_thread" => ClientAction::NewThread,
        "chats_toggle" => ClientAction::ToggleLeft,
        "inspector_toggle" => ClientAction::ToggleRight,
        "toggle_bottom" => ClientAction::ToggleBottom,
        "stop" => ClientAction::Interrupt,
        "create_checkpoint" => ClientAction::CreateCheckpoint,
        "center_tui" => ClientAction::ToggleCenterMode,
        "layout_reset" => ClientAction::ResetOutlook,
        "focus_layout" => ClientAction::FocusLayout,
        "left_section_threads" => ClientAction::SelectLeftSection(LeftSection::Threads),
        "left_section_agents" => ClientAction::SelectLeftSection(LeftSection::Agents),
        "left_section_files" => ClientAction::SelectLeftSection(LeftSection::Files),
        "left_section_activity" => ClientAction::SelectLeftSection(LeftSection::Activity),
        "close_overlay" => ClientAction::CloseOverlay,
        "send" => ClientAction::Send,
        "copy_last_message" => ClientAction::CopyLastMessage,
        "hide_left" => ClientAction::HideLeft,
        "hide_right" => ClientAction::HideRight,
        "hide_bottom" => ClientAction::HideBottom,
        "tab_session" => ClientAction::SelectTab(InspectorTab::Session),
        "tab_cores" => ClientAction::SelectTab(InspectorTab::Resources),
        "tab_mcp" => ClientAction::SelectTab(InspectorTab::Mcp),
        "tab_points" => ClientAction::SelectTab(InspectorTab::Checkpoints),
        "tab_git" => ClientAction::SelectTab(InspectorTab::Git),
        "tab_term" => ClientAction::SelectTab(InspectorTab::Terminal),
        "tab_skills" => ClientAction::SelectTab(InspectorTab::Skills),
        "tab_diff" => ClientAction::SelectTab(InspectorTab::Diff),
        "tab_browser" => ClientAction::SelectTab(InspectorTab::Browser),
        "tab_files" => ClientAction::SelectTab(InspectorTab::Files),
        "tab_activity" => ClientAction::SelectTab(InspectorTab::Activity),
        "tab_agents" => ClientAction::SelectTab(InspectorTab::Agents),
        "toast_dismiss" => ClientAction::DismissToast,
        "copy_session" => ClientAction::CopySession,
        "run_git_status" => ClientAction::RunGitStatus,
        "popout_inspector" => ClientAction::PopOutInspector,
        "dock_inspector" => ClientAction::DockInspector,
        "close_popout" => ClientAction::ClosePopOut,
        "next_region" => ClientAction::NextRegion,
        "nudge_bottom_up" => ClientAction::NudgeBottomUp,
        "nudge_bottom_down" => ClientAction::NudgeBottomDown,
        "allow_once" => ClientAction::ApproveOnce,
        "later" => ClientAction::Later,
        "pin_thread" => ClientAction::PinThread,
        "mark_unread" => ClientAction::MarkUnread,
        "archive_thread" => ClientAction::ArchiveThread,
        "unarchive_thread" => ClientAction::UnarchiveThread,
        "copy_thread_id" => ClientAction::CopyThreadId,
        "settings_about" => ClientAction::OpenAbout,
        _ => return None,
    })
}

/// Hatch chords that stay live while an external Grok TUI is running.
pub fn is_tui_hatch(action: ClientAction) -> bool {
    matches!(
        action,
        ClientAction::ToggleCenterMode
            | ClientAction::SetCenterGui
            | ClientAction::SetCenterTui
            | ClientAction::ResetOutlook
            | ClientAction::ToggleLeft
            | ClientAction::ToggleRight
            | ClientAction::TogglePalette
            | ClientAction::ToggleBottom
            | ClientAction::ToggleSettings
            | ClientAction::CloseOverlay
            | ClientAction::FocusLayout
            | ClientAction::ToggleHelp
            | ClientAction::PopOutInspector
            | ClientAction::DockInspector
            | ClientAction::ClosePopOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_roundtrip_and_normalize() {
        let p = Chord::parse("ctrl-shift-p").unwrap();
        assert!(p.ctrl && p.shift && !p.alt);
        assert_eq!(p.key, "p");
        assert_eq!(p.encode(), "ctrl-shift-p");
        assert_eq!(Chord::parse("CTRL-SHIFT-P"), Some(p.clone()));
        assert_ne!(Chord::parse("ctrl-p").unwrap(), p);

        let tick = Chord::parse("ctrl-`").unwrap();
        assert_eq!(tick.encode(), "ctrl-`");
        let oem = Chord::new("oem_3", true, false, false);
        assert_eq!(oem, tick);

        assert_eq!(Chord::parse(""), None);
        assert_eq!(Chord::parse("   "), None);
        assert_eq!(Chord::parse("ctrl-"), None);
        assert_eq!(Chord::parse("ctrl-shift-p-x"), None);
        assert!(Chord::parse("f1").unwrap().key == "f1");
        assert_eq!(Chord::parse("ctrl-,").unwrap().encode(), "ctrl-,");
        assert_eq!(Chord::parse("escape").unwrap().encode(), "escape");
        let alt = Chord::parse("ctrl-alt-shift-l").unwrap();
        assert_eq!(alt.encode(), "ctrl-shift-alt-l");
    }

    #[test]
    fn defaults_split_ctrl_p_and_shift_p() {
        let t = BindingTable::defaults();
        assert_eq!(t.lookup_spec("ctrl-k"), Some(ClientAction::TogglePalette));
        assert_eq!(
            t.lookup_spec("ctrl-shift-p"),
            Some(ClientAction::TogglePalette)
        );
        assert_eq!(t.lookup_spec("ctrl-p"), Some(ClientAction::ToggleSearch));
        assert_eq!(
            t.lookup_spec("ctrl-shift-f"),
            Some(ClientAction::ToggleSearch)
        );
        assert_eq!(t.lookup_spec("ctrl-,"), Some(ClientAction::ToggleSettings));
        assert_eq!(t.lookup_spec("f2"), Some(ClientAction::ToggleSettings));
        assert_ne!(t.lookup_spec("ctrl-p"), t.lookup_spec("ctrl-shift-p"));
        assert_eq!(
            t.lookup_spec("ctrl-1"),
            Some(ClientAction::SelectLeftSection(LeftSection::Threads))
        );
        assert_eq!(
            t.lookup_spec("ctrl-4"),
            Some(ClientAction::SelectLeftSection(LeftSection::Activity))
        );
        assert_eq!(t.lookup_spec("escape"), Some(ClientAction::CloseOverlay));
        assert_eq!(
            t.lookup_spec("ctrl-shift-d"),
            Some(ClientAction::PopOutInspector)
        );
        assert_eq!(
            t.lookup_spec("ctrl-shift-e"),
            Some(ClientAction::DockInspector)
        );
        assert_eq!(t.lookup_spec("ctrl-w"), Some(ClientAction::ClosePopOut));
        assert_eq!(t.lookup_spec("ctrl-tab"), Some(ClientAction::NextRegion));
        assert_eq!(
            t.lookup_spec("ctrl-alt-up"),
            Some(ClientAction::NudgeBottomUp)
        );
        assert_eq!(
            t.lookup_spec("ctrl-alt-down"),
            Some(ClientAction::NudgeBottomDown)
        );
        assert_eq!(
            t.lookup_spec("ctrl-shift-l"),
            Some(ClientAction::ResetOutlook)
        );
        assert_eq!(t.lookup_spec("ctrl-b"), Some(ClientAction::ToggleLeft));
        assert!(t.len() >= 20);
        assert!(!t.is_empty());
        assert_eq!(BindingTable::default(), t);
        let empty = BindingTable { rows: Vec::new() };
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
        assert_eq!(empty.lookup_spec("ctrl-k"), None);
    }

    #[test]
    fn last_bind_wins() {
        let mut t = BindingTable::defaults();
        t.bind(Chord::parse("ctrl-p").unwrap(), ClientAction::TogglePalette);
        assert_eq!(t.lookup_spec("ctrl-p"), Some(ClientAction::TogglePalette));
        t.bind(Chord::parse("ctrl-p").unwrap(), ClientAction::ToggleSearch);
        assert_eq!(t.lookup_spec("ctrl-p"), Some(ClientAction::ToggleSearch));
        assert_eq!(
            t.lookup(&Chord::new("P", true, false, false)),
            Some(ClientAction::ToggleSearch)
        );
    }

    #[test]
    fn from_pairs_overlays_defaults() {
        let pairs = vec![
            ("ctrl-p".into(), "command_palette".into()),
            ("nope".into(), "send".into()),
            ("ctrl-k".into(), "not-an-action".into()),
        ];
        let t = BindingTable::from_pairs(&pairs);
        assert_eq!(t.lookup_spec("ctrl-p"), Some(ClientAction::TogglePalette));
        assert_eq!(t.lookup_spec("ctrl-k"), Some(ClientAction::TogglePalette));
        assert_eq!(t.lookup_spec("f1"), Some(ClientAction::ToggleHelp));
        let encoded = t.pairs();
        assert!(encoded
            .iter()
            .any(|(c, a)| c == "ctrl-p" && a == "command_palette"));
    }

    #[test]
    fn action_id_roundtrip_defaults() {
        for (spec, action) in DEFAULT_BINDINGS {
            let id = action_id(*action).unwrap_or_else(|| panic!("id for {spec}"));
            let back = action_from_id(id).unwrap_or_else(|| panic!("from {id}"));
            assert_eq!(back, *action, "{spec} {id}");
        }
        assert_eq!(
            action_from_id("toggle_palette"),
            Some(ClientAction::TogglePalette)
        );
        assert_eq!(action_from_id("missing"), None);
        assert_eq!(action_id(ClientAction::SelectThread(2)), None);
        assert_eq!(action_id(ClientAction::Send), Some("send"));
        assert_ne!(
            action_id(ClientAction::Send),
            action_id(ClientAction::Interrupt)
        );
    }

    #[test]
    fn tui_hatch_is_narrow() {
        assert!(is_tui_hatch(ClientAction::ToggleCenterMode));
        assert!(is_tui_hatch(ClientAction::TogglePalette));
        assert!(is_tui_hatch(ClientAction::ToggleSettings));
        assert!(is_tui_hatch(ClientAction::ResetOutlook));
        assert!(!is_tui_hatch(ClientAction::NewThread));
        assert!(!is_tui_hatch(ClientAction::Send));
        assert!(!is_tui_hatch(ClientAction::ToggleSearch));
    }
}
