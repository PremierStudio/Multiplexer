//! Pure desktop chrome for Phase 0.4.
//!
//! No GPUI types live here. The desktop binary projects [`DesktopChrome`]
//! into a window. Tests and CI stay headless.

mod actions;
mod agents;
mod approval_ui;
mod article;
mod bars;
mod bindings;
mod catalog;
mod center;
mod chrome_geom;
mod composer;
mod diff_view;
mod fuzzy;
mod icons;
mod inspector_model;
mod integrations;
mod keymap;
mod menus;
mod notices;
mod overlay;
mod palette;
mod persist;
mod remote;
mod review;
mod search;
mod settings;
mod slash;
mod status;
mod terminal_ui;
mod usage;
mod widgets;
mod workbench;
mod workspace;

pub use actions::{apply_layout_action, ClientAction};
pub use agents::{
    orchestration_list, orchestration_list_json, orchestration_spawn_missing, AgentRow,
    OrchestrationList, ThreadStatus,
};
pub use approval_ui::PendingApproval;
pub use article::{parse_article, parse_inlines, ArticleBlock, InlineSpan};
pub use bars::usage_bar;
pub use bindings::{host_call, worktree_create_call, ActionContext, HostCall};
pub use catalog::{
    format_ram, merge_models, open_external_program, parse_model_keys, parse_tailscale_dns,
    remotes_serve_note, slash_arg,
};
pub use center::{CenterMode, GrokTuiHost, TuiLife};
pub use chrome_geom::{bottom_height_from_mouse, remotes_pill_label, title_overflow};
pub use composer::{
    clamp_cursor, delete_back, delete_forward, insert_at, move_end, move_home, move_left,
    move_right, move_word_left, move_word_right,
};
pub use diff_view::{
    changes_headline, mark_last_turn, parse_porcelain, sort_diffs, status_mark, DiffRow, DiffSort,
};
pub use fuzzy::{fuzzy_best, fuzzy_score};
pub use icons::{BrandIcon, ChromeGlyph};
pub use inspector_model::{inspector_rows, row_detail};
pub use integrations::{filter_tiles, integration_tiles, TileSpec};
pub use keymap::{action_from_id, action_id, is_tui_hatch, BindingTable, Chord};
pub use menus::{items as menu_items, menu_for, MenuItem, MenuKind, OpenMenu};
pub use notices::{
    auto_dismisses, dismiss_newest, dismiss_notice, push_notice, visible_notices, Notice,
    NoticeKind, NOTICE_AUTO_MS, NOTICE_CAP, NOTICE_PAINT,
};
pub use overlay::{OverlayFlags, OverlayKind};
pub use palette::{
    default_items, filter_items, hit_action, palette_hits, pane_items, PaletteItem, PaletteState,
};
pub use persist::{
    about_info, apply_deep_link, crash_journal_from_json, crash_journal_to_json,
    crash_restore_notice, default_crash_path, default_first_run_path, default_layout_path,
    first_run_completed, first_run_keychain_notice, journal_from_workspace, layout_from_json,
    layout_to_json, leaf_name, parse_deep_link, read_crash_journal, read_layout, thread_leaf_title,
    write_crash_journal, write_first_run_done, write_layout, AboutInfo, CrashJournal, DeepLink,
    LayoutPersist,
};
pub use remote::{detect_remotes, RemoteRow};
pub use review::{
    cap_text, context_strip, git_ahead_behind, git_diff_line, git_dirty, git_header,
    handshake_state, merge_cores, working_copy, DIFF_TEXT_CAP,
};
pub use search::{search_workspace, SearchHit, SearchKind};
pub use settings::{
    default_settings_path, read_settings, settings_from_json, settings_to_json, write_settings,
    SettingsSection, UiSettings,
};
pub use slash::{parse_slash, plan_send, slash_hint, SendPlan, SlashCommand};
pub use status::{status_from, status_line, ClientStatus};
pub use terminal_ui::{
    format_line, help_text, parse_builtin, push_capped, visible_tail, BuiltinCmd, TermLineKind,
    TERM_HISTORY_MAX, TERM_PROMPT,
};
pub use usage::UsageSnapshot;
pub use widgets::{
    empty_state_tiles, BadgeSpec, ButtonKind, ButtonSpec, EmptyStateSpec, ListRowSpec, TabSpec,
    Tone, HEIGHT_CARD, HEIGHT_RAIL, HEIGHT_TITLE,
};
pub use workbench::{
    activity_items, default_browser_candidates, detect_browsers, filter_files, join_project_path,
    merge_mcp, preferred_browser, ActivityItem,
};
pub use workspace::{
    ChatMessage, CheckpointRow, ChromeLayout, CoreRow, FocusRegion, InspectorTab, LeftSection,
    McpLife, McpRow, RailVis, Role, SkillItem, Thread, Workspace, WorktreeCard,
    BOTTOM_HEIGHT_COLLAPSED, BOTTOM_HEIGHT_EXPANDED, BOTTOM_HEIGHT_OPEN_MIN, LEFT_WIDTH_MAX,
    LEFT_WIDTH_MIN, RAIL_COLLAPSED, RIGHT_WIDTH_MAX, RIGHT_WIDTH_MIN, TITLE_HEIGHT,
};

use multiplexer_layout::{LayoutForest, LayoutNode, PaneId};

/// Title of the primary Multiplexer window.
pub const DEFAULT_WINDOW_TITLE: &str = "Multiplexer";

/// How the chrome is attached to a Multiplexer server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    /// Hello and ping succeeded. No session id yet.
    Ready,
    Connected {
        session_ids: Vec<String>,
    },
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected { session_ids } if !session_ids.is_empty())
    }

    pub fn session_count(&self) -> usize {
        match self {
            ConnectionState::Connected { session_ids } => session_ids.len(),
            _ => 0,
        }
    }

    pub fn status_label(&self) -> &'static str {
        match self {
            ConnectionState::Disconnected => "disconnected",
            ConnectionState::Connecting => "connecting",
            ConnectionState::Ready => "ready",
            ConnectionState::Connected { session_ids } if session_ids.is_empty() => "ready",
            ConnectionState::Connected { .. } => "connected",
        }
    }
}

/// OS-window chrome: title plus the pane forest to project.
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopChrome {
    pub title: String,
    pub layout: LayoutForest,
    pub connection: ConnectionState,
}

impl DesktopChrome {
    /// Outlook-style three-pane chrome for the primary window.
    pub fn default_outlook() -> Self {
        Self {
            title: DEFAULT_WINDOW_TITLE.to_owned(),
            layout: LayoutForest::default_outlook(),
            connection: ConnectionState::Disconnected,
        }
    }

    /// Live (non-ghost) pane ids in window order, left-to-right / top-to-bottom.
    pub fn live_pane_ids(&self) -> Vec<PaneId> {
        let mut ids = Vec::new();
        for window in self.layout.windows() {
            collect_live(&window.root, &mut ids);
        }
        ids
    }

    /// Number of live panes across every window.
    pub fn live_pane_count(&self) -> usize {
        self.live_pane_ids().len()
    }

    /// Hello-frame copy projected into the blank center of the shell.
    pub fn hello_frame_label(&self) -> String {
        format!("Hello, {}", self.title)
    }

    /// Status-bar copy: title plus connection state.
    pub fn connection_label(&self) -> String {
        format!("{} · {}", self.title, self.connection.status_label())
    }

    pub fn mark_connecting(&mut self) {
        self.connection = ConnectionState::Connecting;
    }

    pub fn mark_connected(&mut self, session_ids: Vec<String>) {
        if session_ids.is_empty() {
            self.connection = ConnectionState::Ready;
        } else {
            self.connection = ConnectionState::Connected { session_ids };
        }
    }

    pub fn mark_disconnected(&mut self) {
        self.connection = ConnectionState::Disconnected;
    }
}

fn collect_live(node: &LayoutNode, out: &mut Vec<PaneId>) {
    match node {
        LayoutNode::Leaf { pane, ghost, .. } if !*ghost => out.push(*pane),
        LayoutNode::Leaf { .. } => {}
        LayoutNode::Split(split) => {
            collect_live(&split.first, out);
            collect_live(&split.second, out);
        }
    }
}

#[cfg(test)]
mod unit {
    use super::*;
    use multiplexer_layout::PaneId;

    #[test]
    fn default_title_is_multiplexer_literal() {
        assert_eq!(DEFAULT_WINDOW_TITLE, "Multiplexer");
        assert_eq!(DesktopChrome::default_outlook().title, "Multiplexer");
        assert_ne!(DesktopChrome::default_outlook().title, "multiplexer");
        assert_ne!(DesktopChrome::default_outlook().title, "");
    }

    #[test]
    fn default_layout_matches_outlook_forest() {
        let chrome = DesktopChrome::default_outlook();
        assert_eq!(chrome.layout, LayoutForest::default_outlook());
    }

    #[test]
    fn default_layout_has_three_live_panes() {
        let chrome = DesktopChrome::default_outlook();
        assert_eq!(chrome.live_pane_count(), 3);
        assert_eq!(
            chrome.live_pane_ids(),
            vec![PaneId(1), PaneId(2), PaneId(3)]
        );
    }

    #[test]
    fn hello_frame_uses_title() {
        let chrome = DesktopChrome::default_outlook();
        assert_eq!(chrome.hello_frame_label(), "Hello, Multiplexer");
        let custom = DesktopChrome {
            title: "Other".to_owned(),
            layout: LayoutForest::default_outlook(),
            connection: ConnectionState::Disconnected,
        };
        assert_eq!(custom.hello_frame_label(), "Hello, Other");
    }

    #[test]
    fn close_drops_live_count_and_omits_closed_id() {
        let mut chrome = DesktopChrome::default_outlook();
        chrome.layout.close(PaneId(1)).unwrap();
        assert_eq!(chrome.live_pane_count(), 2);
        assert_eq!(chrome.live_pane_ids(), vec![PaneId(2), PaneId(3)]);
    }

    #[test]
    fn detach_keeps_live_count_and_ignores_ghost() {
        let mut chrome = DesktopChrome::default_outlook();
        chrome.layout.detach(PaneId(3)).unwrap();
        assert_eq!(chrome.layout.window_count(), 2);
        assert_eq!(chrome.live_pane_count(), 3);
        assert_eq!(
            chrome.live_pane_ids(),
            vec![PaneId(1), PaneId(2), PaneId(3)]
        );
    }

    #[test]
    fn default_connection_is_disconnected() {
        let chrome = DesktopChrome::default_outlook();
        assert_eq!(chrome.connection, ConnectionState::Disconnected);
        assert!(!chrome.connection.is_connected());
        assert_eq!(chrome.connection.session_count(), 0);
        assert_eq!(chrome.connection_label(), "Multiplexer · disconnected");
    }

    #[test]
    fn connection_lifecycle_updates_label_and_count() {
        let mut chrome = DesktopChrome::default_outlook();
        chrome.mark_connecting();
        assert_eq!(chrome.connection.status_label(), "connecting");
        chrome.mark_connected(vec!["sess-1".into(), "sess-2".into()]);
        assert!(chrome.connection.is_connected());
        assert_eq!(chrome.connection.session_count(), 2);
        assert_eq!(chrome.connection_label(), "Multiplexer · connected");
        chrome.mark_disconnected();
        assert!(!chrome.connection.is_connected());
        assert_eq!(chrome.connection.session_count(), 0);
    }
}
