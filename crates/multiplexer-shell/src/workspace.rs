//! Headless workspace model: threads, transcript, composer, inspector.

use crate::approval_ui::PendingApproval;
use crate::composer::{clamp_cursor, delete_back, insert_at};
use crate::ConnectionState;

/// Who wrote a transcript line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

/// One chat line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
}

/// One agent thread in the left rail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    pub id: String,
    pub title: String,
    pub messages: Vec<ChatMessage>,
    pub status: String,
    pub model: String,
    pub pinned: bool,
    pub unread: bool,
    pub archived: bool,
}

/// One skill inventory row. Enable is a local flag, not loaded into grok.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillItem {
    pub name: String,
    pub source: String,
    pub enabled: bool,
    pub preview: String,
}

/// Right-rail inspector tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorTab {
    Session,
    Resources,
    Mcp,
    Checkpoints,
    Git,
    Terminal,
    Skills,
    Files,
    Activity,
    Agents,
    Diff,
    Browser,
}

impl InspectorTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Resources => "Cores",
            Self::Mcp => "MCP",
            Self::Checkpoints => "Points",
            Self::Git => "Git",
            Self::Terminal => "Term",
            Self::Skills => "Skills",
            Self::Files => "Files",
            Self::Activity => "Activity",
            Self::Agents => "Agents",
            Self::Diff => "Diffs",
            Self::Browser => "Browser",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Session => "◎",
            Self::Resources => "▣",
            Self::Mcp => "⬡",
            Self::Checkpoints => "⚑",
            Self::Git => "⎇",
            Self::Terminal => ">_",
            Self::Skills => "✦",
            Self::Files => "▤",
            Self::Activity => "●",
            Self::Agents => "⚡",
            Self::Diff => "±",
            Self::Browser => "⧉",
        }
    }

    pub fn all() -> [InspectorTab; 12] {
        [
            Self::Session,
            Self::Resources,
            Self::Mcp,
            Self::Checkpoints,
            Self::Git,
            Self::Terminal,
            Self::Skills,
            Self::Files,
            Self::Activity,
            Self::Agents,
            Self::Diff,
            Self::Browser,
        ]
    }
}

/// Left Outlook section. Icon-rail labels differ from enum names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftSection {
    Threads,
    Agents,
    Files,
    Activity,
}

impl LeftSection {
    pub fn all() -> [LeftSection; 4] {
        [Self::Threads, Self::Agents, Self::Files, Self::Activity]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Threads => "Threads",
            Self::Agents => "Agents",
            Self::Files => "Files",
            Self::Activity => "Activity",
        }
    }

    pub fn rail_label(self) -> &'static str {
        match self {
            Self::Threads => "Chats",
            Self::Agents => "Agents",
            Self::Files => "Projects",
            Self::Activity => "Activity",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Threads => "☰",
            Self::Agents => "⚡",
            Self::Files => "▤",
            Self::Activity => "●",
        }
    }
}

/// One logical CPU sample for the inspector.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreRow {
    pub index: usize,
    pub usage: f32,
    pub reserved: bool,
}

/// Supervised MCP lifecycle projection (not a real child this wave).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McpLife {
    #[default]
    Stopped,
    Ready,
    Crashed,
    Failed,
}

impl McpLife {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Ready => "ready",
            Self::Crashed => "crashed",
            Self::Failed => "failed",
        }
    }
}

/// One configured MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRow {
    pub name: String,
    pub command: String,
    pub transport: String,
    pub state: McpLife,
}

/// One checkpoint row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointRow {
    pub id: String,
    pub label: String,
}

/// One linked worktree card (path plus optional branch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeCard {
    pub path: String,
    pub branch: Option<String>,
}

/// Min/max pixel widths for the Outlook rails.
pub const LEFT_WIDTH_MIN: f32 = 180.0;
pub const LEFT_WIDTH_MAX: f32 = 420.0;
pub const RIGHT_WIDTH_MIN: f32 = 220.0;
pub const RIGHT_WIDTH_MAX: f32 = 480.0;
pub const RAIL_COLLAPSED: f32 = 48.0;
pub const TITLE_HEIGHT: f32 = 48.0;
pub const BOTTOM_HEIGHT_COLLAPSED: f32 = 36.0;
pub const BOTTOM_HEIGHT_EXPANDED: f32 = 240.0;
pub const BOTTOM_HEIGHT_OPEN_MIN: f32 = 80.0;
pub const BOTTOM_HEIGHT_MIN: f32 = BOTTOM_HEIGHT_OPEN_MIN;
pub const BOTTOM_HEIGHT_MAX: f32 = 480.0;

/// Left or right rail visibility. Hidden occupies 0. IconRail occupies 44.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RailVis {
    #[default]
    Open,
    IconRail,
    Hidden,
}

impl RailVis {
    pub fn occupied(self, open_width: f32) -> f32 {
        match self {
            Self::Open => open_width,
            Self::IconRail => RAIL_COLLAPSED,
            Self::Hidden => 0.0,
        }
    }

    /// Title bar: Open <-> IconRail. Hidden restores to IconRail.
    pub fn cycle_icon(self) -> Self {
        match self {
            Self::Open => Self::IconRail,
            Self::IconRail => Self::Open,
            Self::Hidden => Self::IconRail,
        }
    }
}

/// Snapshot used by focus layout restore.
#[derive(Debug, Clone, PartialEq)]
struct ChromeSnapshot {
    chrome: ChromeLayout,
    bottom_open: bool,
    bottom_hidden: bool,
    bottom_height: f32,
    last_open_height: f32,
}

/// Show/hide and width of the left and right rails.
#[derive(Debug, Clone, PartialEq)]
pub struct ChromeLayout {
    pub left: RailVis,
    pub right: RailVis,
    pub left_width: f32,
    pub right_width: f32,
}

impl Default for ChromeLayout {
    fn default() -> Self {
        Self {
            left: RailVis::Open,
            right: RailVis::Open,
            left_width: 248.0,
            right_width: 300.0,
        }
    }
}

impl ChromeLayout {
    pub fn left_open(&self) -> bool {
        self.left == RailVis::Open
    }

    pub fn right_open(&self) -> bool {
        self.right == RailVis::Open
    }

    pub fn toggle_left(&mut self) {
        self.left = self.left.cycle_icon();
    }

    pub fn toggle_right(&mut self) {
        self.right = self.right.cycle_icon();
    }

    pub fn hide_left(&mut self) {
        self.left = RailVis::Hidden;
    }

    pub fn hide_right(&mut self) {
        self.right = RailVis::Hidden;
    }

    pub fn open_left(&mut self) {
        self.left = RailVis::Open;
    }

    pub fn open_right(&mut self) {
        self.right = RailVis::Open;
    }

    pub fn set_left_width(&mut self, width: f32) {
        self.left_width = width.clamp(LEFT_WIDTH_MIN, LEFT_WIDTH_MAX);
        self.left = RailVis::Open;
    }

    pub fn set_right_width(&mut self, width: f32) {
        self.right_width = width.clamp(RIGHT_WIDTH_MIN, RIGHT_WIDTH_MAX);
        self.right = RailVis::Open;
    }

    pub fn nudge_left(&mut self, delta: f32) {
        self.set_left_width(self.left_width + delta);
    }

    pub fn nudge_right(&mut self, delta: f32) {
        self.set_right_width(self.right_width + delta);
    }

    /// Width the left rail occupies. Hidden is 0.
    pub fn occupied_left(&self) -> f32 {
        self.left.occupied(self.left_width)
    }

    pub fn occupied_right(&self) -> f32 {
        self.right.occupied(self.right_width)
    }
}

/// Product workspace: chats + composer + inspector. No GPUI types.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub project: String,
    pub model: String,
    pub connection: ConnectionState,
    pub threads: Vec<Thread>,
    pub selected: usize,
    pub draft: String,
    pub inspector: InspectorTab,
    pub worktrees: Vec<String>,
    pub chrome: ChromeLayout,
    pub cores: Vec<CoreRow>,
    pub mcp: Vec<McpRow>,
    pub checkpoints: Vec<CheckpointRow>,
    pub reminder: Option<(String, String)>,
    pub terminal_log: Vec<String>,
    pub busy: bool,
    pub pending: Option<PendingApproval>,
    pub cursor: usize,
    pub models: Vec<String>,
    pub files: Vec<String>,
    pub skills: Vec<String>,
    pub git_status: String,
    pub term_draft: String,
    pub palette_open: bool,
    pub help_open: bool,
    pub selected_checkpoint: Option<String>,
    pub selected_worktree: Option<usize>,
    pub left_section: LeftSection,
    pub right_expanded_id: Option<String>,
    pub bottom_open: bool,
    pub bottom_hidden: bool,
    pub bottom_height: f32,
    pub last_open_height: f32,
    pub selected_file: Option<String>,
    pub notices: Vec<crate::notices::Notice>,
    pub settings: crate::settings::UiSettings,
    pub wt_path: String,
    pub wt_branch: String,
    pub wt_create_branch: bool,
    pub settings_open: bool,
    pub search_open: bool,
    pub search_query: String,
    pub search_selected: usize,
    pub settings_section: crate::settings::SettingsSection,
    pub recent_commands: Vec<String>,
    pub file_filter: String,
    pub thread_drafts: Vec<(String, String, usize)>,
    pub file_expanded: Vec<String>,
    pub usage_turns: u64,
    pub usage_tokens: u64,
    pub center_mode: crate::center::CenterMode,
    pub grok_tui: crate::center::GrokTuiHost,
    pub diff_rows: Vec<crate::diff_view::DiffRow>,
    pub diff_sort: crate::diff_view::DiffSort,
    pub last_turn_paths: Vec<String>,
    pub selected_diff: Option<String>,
    pub diff_text: String,
    pub worktree_cards: Vec<WorktreeCard>,
    pub hello_ok: bool,
    pub ping_ok: bool,
    pub selected_mcp: Option<String>,
    pub skill_items: Vec<SkillItem>,
    pub hooks: Vec<(String, String)>,
    pub term_cwd: String,
    pub last_error: String,
    pub ram_bytes: u64,
    pub browser_url: String,
    pub forest: multiplexer_layout::LayoutForest,
    pub inspector_popped: bool,
    pub focus_region: FocusRegion,
    pub context_menu: Option<crate::menus::OpenMenu>,
    pub first_run_open: bool,
    pub git_checkpoints: bool,
    next_id: u64,
    next_notice: u64,
    focus_snapshot: Option<ChromeSnapshot>,
}

/// Ctrl+Tab walk: left list, center host, inspector, terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRegion {
    Left,
    Center,
    Right,
    Bottom,
}

impl FocusRegion {
    pub fn next(self) -> Self {
        match self {
            Self::Left => Self::Center,
            Self::Center => Self::Right,
            Self::Right => Self::Bottom,
            Self::Bottom => Self::Left,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "inspector",
            Self::Bottom => "terminal",
        }
    }
}

impl Workspace {
    pub fn new(project: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let project = project.into();
        let mut ws = Self {
            project: project.clone(),
            model: model.clone(),
            connection: ConnectionState::Disconnected,
            threads: Vec::new(),
            selected: 0,
            draft: String::new(),
            inspector: InspectorTab::Session,
            worktrees: Vec::new(),
            chrome: ChromeLayout::default(),
            cores: Vec::new(),
            mcp: Vec::new(),
            checkpoints: Vec::new(),
            reminder: None,
            terminal_log: Vec::new(),
            busy: false,
            pending: None,
            cursor: 0,
            models: vec![model],
            files: Vec::new(),
            skills: Vec::new(),
            git_status: String::new(),
            term_draft: String::new(),
            palette_open: false,
            help_open: false,
            selected_checkpoint: None,
            selected_worktree: None,
            left_section: LeftSection::Threads,
            right_expanded_id: None,
            bottom_open: false,
            bottom_hidden: false,
            bottom_height: BOTTOM_HEIGHT_COLLAPSED,
            last_open_height: BOTTOM_HEIGHT_EXPANDED,
            selected_file: None,
            notices: Vec::new(),
            settings: crate::settings::UiSettings::default(),
            wt_path: "../mux-feat".into(),
            wt_branch: "feat".into(),
            wt_create_branch: true,
            settings_open: false,
            search_open: false,
            search_query: String::new(),
            search_selected: 0,
            settings_section: crate::settings::SettingsSection::Appearance,
            recent_commands: Vec::new(),
            file_filter: String::new(),
            thread_drafts: Vec::new(),
            file_expanded: Vec::new(),
            usage_turns: 0,
            usage_tokens: 0,
            center_mode: crate::center::CenterMode::Gui,
            grok_tui: crate::center::GrokTuiHost::idle(project.clone()),
            diff_rows: Vec::new(),
            diff_sort: crate::diff_view::DiffSort::LastTurn,
            last_turn_paths: Vec::new(),
            selected_diff: None,
            diff_text: String::new(),
            worktree_cards: Vec::new(),
            hello_ok: false,
            ping_ok: false,
            selected_mcp: None,
            skill_items: Vec::new(),
            hooks: Vec::new(),
            term_cwd: project.clone(),
            last_error: String::new(),
            ram_bytes: 0,
            browser_url: String::new(),
            forest: multiplexer_layout::LayoutForest::default_outlook(),
            inspector_popped: false,
            focus_region: FocusRegion::Center,
            context_menu: None,
            first_run_open: false,
            git_checkpoints: false,
            next_id: 1,
            next_notice: 1,
            focus_snapshot: None,
        };
        ws.new_thread();
        ws
    }

    pub fn title_bar(&self) -> String {
        format!(
            "Multiplexer  ·  {}  ·  {}  ·  {}",
            self.project,
            self.model,
            self.connection.status_label()
        )
    }

    pub fn selected_thread(&self) -> Option<&Thread> {
        self.threads.get(self.selected)
    }

    pub fn selected_thread_mut(&mut self) -> Option<&mut Thread> {
        self.threads.get_mut(self.selected)
    }

    pub fn new_thread(&mut self) -> String {
        self.stash_selected_draft();
        let id = format!("thr-{}", self.next_id);
        self.next_id += 1;
        self.threads.push(Thread {
            id: id.clone(),
            title: "New chat".to_owned(),
            messages: Vec::new(),
            status: "idle".to_owned(),
            model: self.model.clone(),
            pinned: false,
            unread: false,
            archived: false,
        });
        self.selected = self.threads.len() - 1;
        self.draft.clear();
        self.cursor = 0;
        id
    }

    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.threads.len() || index == self.selected {
            return false;
        }
        self.stash_selected_draft();
        self.selected = index;
        self.restore_selected_draft();
        true
    }

    fn stash_selected_draft(&mut self) {
        let Some(id) = self.threads.get(self.selected).map(|t| t.id.clone()) else {
            return;
        };
        self.thread_drafts.retain(|(i, _, _)| i != &id);
        if !self.draft.is_empty() {
            self.thread_drafts
                .push((id, self.draft.clone(), self.cursor));
        }
    }

    fn restore_selected_draft(&mut self) {
        let Some(id) = self.threads.get(self.selected).map(|t| t.id.clone()) else {
            self.draft.clear();
            self.cursor = 0;
            return;
        };
        if let Some((_, d, c)) = self
            .thread_drafts
            .iter()
            .find(|(i, _, _)| i == &id)
            .cloned()
        {
            self.draft = d;
            self.cursor = c.min(self.draft.chars().count());
        } else {
            self.draft.clear();
            self.cursor = 0;
        }
    }

    /// Remove a thread. Keeps at least one. If the selected thread is
    /// removed, the previous index (or 0) becomes selected.
    pub fn delete_thread(&mut self, index: usize) -> bool {
        if self.threads.len() <= 1 || index >= self.threads.len() {
            return false;
        }
        let dropped = self.threads[index].id.clone();
        self.threads.remove(index);
        self.thread_drafts.retain(|(i, _, _)| i != &dropped);
        if index < self.selected {
            self.selected -= 1;
        } else if index == self.selected {
            self.selected = self.selected.saturating_sub(1);
            self.restore_selected_draft();
        }
        true
    }

    pub fn rename_thread(&mut self, index: usize, title: impl Into<String>) -> bool {
        match self.threads.get_mut(index) {
            Some(thread) => {
                thread.title = title.into();
                true
            }
            None => false,
        }
    }

    pub fn set_draft(&mut self, text: impl Into<String>) {
        self.draft = text.into();
        self.cursor = self.draft.chars().count();
    }

    pub fn type_char(&mut self, c: char) {
        if !c.is_control() {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            self.cursor = insert_at(&mut self.draft, self.cursor, s);
        }
    }

    pub fn backspace(&mut self) {
        self.cursor = delete_back(&mut self.draft, self.cursor);
    }

    pub fn move_cursor_left(&mut self) {
        self.cursor = clamp_cursor(&self.draft, self.cursor);
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor = clamp_cursor(&self.draft, self.cursor);
        let end = self.draft.chars().count();
        if self.cursor < end {
            self.cursor += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor = self.draft.chars().count();
    }

    /// Take the draft as a user message. Returns the text if it was non-empty.
    pub fn send_draft(&mut self) -> Option<String> {
        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return None;
        }
        self.draft.clear();
        self.cursor = 0;
        if let Some(thread) = self.selected_thread_mut() {
            if thread.title == "New chat" {
                thread.title = text.chars().take(40).collect();
            }
            thread.messages.push(ChatMessage {
                role: Role::User,
                text: text.clone(),
            });
            thread.status = "running".to_owned();
        }
        self.busy = true;
        self.usage_turns = self.usage_turns.saturating_add(1);
        Some(text)
    }

    pub fn push_assistant(&mut self, text: impl Into<String>) {
        if let Some(thread) = self.selected_thread_mut() {
            thread.messages.push(ChatMessage {
                role: Role::Assistant,
                text: text.into(),
            });
            thread.status = "idle".to_owned();
        }
        self.busy = false;
    }

    pub fn mark_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.last_error = message.clone();
        if let Some(thread) = self.selected_thread_mut() {
            thread.status = "error".to_owned();
            thread.messages.push(ChatMessage {
                role: Role::Assistant,
                text: message,
            });
        }
        self.busy = false;
    }

    pub fn mark_interrupted(&mut self) {
        self.busy = false;
        if let Some(thread) = self.selected_thread_mut() {
            thread.status = "idle".to_owned();
            thread.messages.push(ChatMessage {
                role: Role::Assistant,
                text: "(interrupted)".into(),
            });
        }
    }

    pub fn push_terminal(&mut self, line: impl Into<String>) {
        self.terminal_log.push(line.into());
        if self.terminal_log.len() > 80 {
            let drop = self.terminal_log.len() - 80;
            self.terminal_log.drain(0..drop);
        }
    }

    pub fn set_reminder(&mut self, branch: impl Into<String>, path: impl Into<String>) {
        self.reminder = Some((branch.into(), path.into()));
    }

    pub fn dismiss_reminder(&mut self) {
        self.reminder = None;
    }

    pub fn connect(&mut self, session_ids: Vec<String>) {
        if session_ids.is_empty() {
            self.connection = ConnectionState::Ready;
        } else {
            self.connection = ConnectionState::Connected { session_ids };
        }
    }

    /// Rotate `models`, assign `self.model`, and return the new model id.
    pub fn cycle_model(&mut self) -> String {
        if self.models.is_empty() {
            self.models.push(self.model.clone());
            return self.model.clone();
        }
        let i = self
            .models
            .iter()
            .position(|m| m == &self.model)
            .unwrap_or(0);
        let next = (i + 1) % self.models.len();
        self.model = self.models[next].clone();
        self.model.clone()
    }

    /// Replace the model catalog. An empty list keeps the current model.
    pub fn set_models(&mut self, models: Vec<String>) {
        if models.is_empty() {
            self.models = vec![self.model.clone()];
            return;
        }
        self.models = models;
        if !self.models.iter().any(|m| m == &self.model) {
            self.model = self.models[0].clone();
        }
    }

    pub fn create_local_checkpoint(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.checkpoints.push(CheckpointRow {
            id: id.into(),
            label: label.into(),
        });
    }

    pub fn select_checkpoint(&mut self, id: Option<String>) {
        self.selected_checkpoint = id;
    }

    pub fn set_files(&mut self, files: Vec<String>) {
        self.files = files;
    }

    pub fn set_skills(&mut self, skills: Vec<String>) {
        self.skills = skills.clone();
        self.skill_items = skills
            .into_iter()
            .map(|raw| {
                let (name, source) = parse_skill_label(&raw);
                SkillItem {
                    name,
                    source,
                    enabled: true,
                    preview: String::new(),
                }
            })
            .collect();
    }

    pub fn set_skill_items(&mut self, items: Vec<SkillItem>) {
        self.skills = items.iter().map(|s| s.name.clone()).collect();
        self.skill_items = items;
    }

    pub fn toggle_skill(&mut self, name: &str) -> bool {
        match self.skill_items.iter_mut().find(|s| s.name == name) {
            Some(item) => {
                item.enabled = !item.enabled;
                true
            }
            None => false,
        }
    }

    pub fn set_git_status(&mut self, status: impl Into<String>) {
        self.git_status = status.into();
    }

    pub fn set_term_draft(&mut self, text: impl Into<String>) {
        self.term_draft = text.into();
    }

    pub fn type_term_char(&mut self, c: char) {
        if !c.is_control() {
            self.term_draft.push(c);
        }
    }

    pub fn backspace_term(&mut self) {
        self.term_draft.pop();
    }

    /// Trim and take the terminal draft. Empty after trim yields `None`.
    pub fn take_term_draft(&mut self) -> Option<String> {
        let text = self.term_draft.trim().to_owned();
        self.term_draft.clear();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn overlay_flags(&self) -> crate::overlay::OverlayFlags {
        crate::overlay::OverlayFlags {
            palette: self.palette_open,
            help: self.help_open,
            settings: self.settings_open,
            search: self.search_open,
        }
    }

    fn apply_overlay_flags(&mut self, flags: crate::overlay::OverlayFlags) {
        self.palette_open = flags.palette;
        self.help_open = flags.help;
        self.settings_open = flags.settings;
        let search_was = self.search_open;
        self.search_open = flags.search;
        if search_was && !flags.search {
            self.search_query.clear();
            self.search_selected = 0;
        }
    }

    pub fn open_overlay(&mut self, kind: crate::overlay::OverlayKind) {
        let mut flags = self.overlay_flags();
        flags.open(kind);
        self.apply_overlay_flags(flags);
    }

    pub fn close_overlay(&mut self, kind: crate::overlay::OverlayKind) {
        let mut flags = self.overlay_flags();
        flags.close(kind);
        self.apply_overlay_flags(flags);
    }

    pub fn toggle_overlay(&mut self, kind: crate::overlay::OverlayKind) {
        let mut flags = self.overlay_flags();
        flags.toggle(kind);
        self.apply_overlay_flags(flags);
    }

    pub fn pop_overlay(&mut self) -> Option<crate::overlay::OverlayKind> {
        let mut flags = self.overlay_flags();
        let popped = flags.pop();
        self.apply_overlay_flags(flags);
        popped
    }

    pub fn toggle_palette(&mut self) {
        self.toggle_overlay(crate::overlay::OverlayKind::Palette);
    }

    pub fn close_palette(&mut self) {
        self.close_overlay(crate::overlay::OverlayKind::Palette);
    }

    pub fn toggle_help(&mut self) {
        self.toggle_overlay(crate::overlay::OverlayKind::Help);
    }

    pub fn remember_command(&mut self, id: &str) {
        if id.is_empty() {
            return;
        }
        self.recent_commands.retain(|x| x != id);
        self.recent_commands.insert(0, id.to_owned());
        if self.recent_commands.len() > 8 {
            self.recent_commands.truncate(8);
        }
    }

    pub fn dismiss_newest_notice(&mut self) -> bool {
        crate::notices::dismiss_newest(&mut self.notices)
    }

    /// Last line preview for the thread list.
    pub fn thread_preview(thread: &Thread) -> String {
        thread
            .messages
            .last()
            .map(|m| {
                let prefix = match m.role {
                    Role::User => "You: ",
                    Role::Assistant => "Agent: ",
                };
                let body: String = m.text.chars().take(48).collect();
                format!("{prefix}{body}")
            })
            .unwrap_or_else(|| "Empty thread".to_owned())
    }

    pub fn session_detail(&self, session_id: Option<&str>) -> String {
        let models = if self.models.is_empty() {
            self.model.clone()
        } else {
            self.models.join(", ")
        };
        format!(
            "Project\n{}\n\nModel\n{}\n\nConnection\n{}\n\nHandshake\nhello {} ping {}\n\nSession\n{}\n\nThreads\n{}\n\nModels\n{}\n\nTurns\n{}\n\nNote\nlocal snapshot only\n\nLast error\n{}\n\nPalette\n{}\n\nHelp\n{}",
            self.project,
            self.model,
            self.connection.status_label(),
            if self.hello_ok { "ok" } else { "no" },
            if self.ping_ok { "ok" } else { "no" },
            session_id.unwrap_or("(none yet)"),
            self.threads.len(),
            models,
            self.usage_turns,
            if self.last_error.is_empty() {
                "(none)"
            } else {
                self.last_error.as_str()
            },
            if self.palette_open { "open" } else { "closed" },
            if self.help_open { "open" } else { "closed" },
        )
    }

    pub fn resource_detail(&self) -> String {
        let mut out = String::from(
            "CPU samples only. Reservation is a local flag. Process containment is not attached.\nNo contained processes.\n",
        );
        out.push_str(&crate::catalog::format_ram(self.ram_bytes));
        out.push_str("\n\n");
        if self.cores.is_empty() {
            out.push_str("Core samples: (waiting)\n");
        } else {
            for c in &self.cores {
                let mark = if c.reserved { "R" } else { " " };
                out.push_str(&format!(
                    "[{mark}] cpu{:<2} {:>5.1}% {}\n",
                    c.index,
                    c.usage,
                    tiny_usage_bar(c.usage)
                ));
            }
        }
        out.push_str("\nWorktrees\n");
        if self.worktrees.is_empty() {
            out.push_str("(none listed)");
        } else {
            out.push_str(&self.worktrees.join("\n"));
        }
        out.push_str("\n\nFiles\n");
        if self.files.is_empty() {
            out.push_str("(none listed)");
        } else {
            out.push_str(&self.files.join("\n"));
        }
        out
    }

    pub fn mcp_detail(&self) -> String {
        if self.mcp.is_empty() {
            return "No MCP servers in ~/.grok/config.toml\nInventory only. No child, no reuse, no teardown.".to_owned();
        }
        self.mcp
            .iter()
            .map(|m| {
                format!(
                    "{}  [{}]  {}\n  {}",
                    m.name,
                    m.transport,
                    m.state.label(),
                    m.command
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn checkpoint_detail(&self) -> String {
        let banner = if self.git_checkpoints {
            "Hidden git refs. Revert restores the working tree."
        } else {
            "Pointer only. Files unchanged."
        };
        if self.checkpoints.is_empty() {
            return format!("{banner}\n\nNo checkpoints yet.");
        }
        let mut out = format!("{banner}\n\n");
        out.push_str(
            &self
                .checkpoints
                .iter()
                .map(|c| {
                    let mark = if self.selected_checkpoint.as_deref() == Some(c.id.as_str()) {
                        "*"
                    } else {
                        " "
                    };
                    format!("{mark} {}  {}", c.id, c.label)
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
        out
    }

    pub fn git_detail(&self) -> String {
        let mut out =
            crate::review::git_header(&self.project, &self.branch_label(), &self.git_status);
        out.push_str("\n\nCreate\n");
        out.push_str(&format!(
            "path {}  branch {}  create_branch {}\n",
            self.wt_path, self.wt_branch, self.wt_create_branch
        ));
        out.push_str("\nWorktrees\n");
        if self.worktree_cards.is_empty() && self.worktrees.is_empty() {
            out.push_str("(none listed)");
        } else if !self.worktree_cards.is_empty() {
            for (i, wt) in self.worktree_cards.iter().enumerate() {
                let mark = if self.selected_worktree == Some(i) {
                    "*"
                } else {
                    " "
                };
                let br = wt.branch.as_deref().unwrap_or("(detached)");
                out.push_str(&format!("{mark} {br}  {}\n", wt.path));
            }
        } else {
            for (i, wt) in self.worktrees.iter().enumerate() {
                let mark = if self.selected_worktree == Some(i) {
                    "*"
                } else {
                    " "
                };
                out.push_str(&format!("{mark} {wt}\n"));
            }
        }
        out.push_str("\n\nStatus\n");
        if self.git_status.is_empty() {
            out.push_str("(none)");
        } else {
            out.push_str(&self.git_status);
        }
        out
    }

    pub fn terminal_detail(&self) -> String {
        let mut out = String::from("Log\n");
        if self.terminal_log.is_empty() {
            out.push_str("(empty)");
        } else {
            out.push_str(&self.terminal_log.join("\n"));
        }
        out.push_str("\n\nDraft\n");
        out.push_str(&self.term_draft);
        out
    }

    pub fn skills_detail(&self) -> String {
        if self.skill_items.is_empty() && self.skills.is_empty() && self.hooks.is_empty() {
            return "No skills found under .grok/skills".to_owned();
        }
        let mut out = String::from("Enable is a local flag (not loaded into grok).\n\n");
        if self.skill_items.is_empty() && self.skills.is_empty() {
            // hooks only
        } else if !self.skill_items.is_empty() {
            for s in &self.skill_items {
                let flag = if s.enabled { "on" } else { "off" };
                out.push_str(&format!("{}  [{}]  {flag}\n", s.name, s.source));
                if !s.preview.is_empty() {
                    out.push_str(&format!("  {}\n", s.preview.lines().next().unwrap_or("")));
                }
            }
        } else {
            out.push_str(&self.skills.join("\n"));
        }
        if !self.hooks.is_empty() {
            out.push_str("\nHooks (list only, not run)\n");
            for (name, when) in &self.hooks {
                out.push_str(&format!("{name}  {when}\n"));
            }
        }
        out
    }

    pub fn files_detail(&self) -> String {
        let shown = crate::workbench::filter_files(&self.files, &self.file_filter);
        if shown.is_empty() {
            "No project files listed.".to_owned()
        } else {
            shown.join("\n")
        }
    }

    pub fn activity_detail(&self) -> String {
        crate::workbench::activity_items(self)
            .into_iter()
            .map(|i| format!("{}  {}", i.title, i.hint))
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn select_left_section(&mut self, section: LeftSection) -> bool {
        if self.left_section == section {
            false
        } else {
            self.left_section = section;
            true
        }
    }

    pub fn toggle_bottom(&mut self) {
        if self.bottom_hidden {
            self.bottom_hidden = false;
            self.bottom_open = false;
            self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
            return;
        }
        if self.bottom_open {
            self.last_open_height = self
                .bottom_height
                .clamp(BOTTOM_HEIGHT_OPEN_MIN, BOTTOM_HEIGHT_MAX);
            self.bottom_open = false;
            self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
        } else {
            self.bottom_open = true;
            self.bottom_height = self
                .last_open_height
                .clamp(BOTTOM_HEIGHT_OPEN_MIN, BOTTOM_HEIGHT_MAX);
        }
    }

    pub fn hide_bottom(&mut self) {
        if self.bottom_open {
            self.last_open_height = self.bottom_height;
        }
        self.bottom_hidden = true;
        self.bottom_open = false;
    }

    pub fn set_bottom_height(&mut self, height: f32) {
        self.bottom_hidden = false;
        if height < BOTTOM_HEIGHT_MIN {
            self.bottom_open = false;
            self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
        } else {
            self.bottom_height = height.clamp(BOTTOM_HEIGHT_MIN, BOTTOM_HEIGHT_MAX);
            self.bottom_open = true;
            self.last_open_height = self.bottom_height;
        }
    }

    pub fn occupied_bottom(&self) -> f32 {
        if self.bottom_hidden {
            0.0
        } else if self.bottom_open {
            self.bottom_height
        } else {
            BOTTOM_HEIGHT_COLLAPSED
        }
    }

    pub fn is_focus_layout(&self) -> bool {
        self.chrome.left == RailVis::Hidden && self.chrome.right == RailVis::Hidden
    }

    pub fn focus_layout(&mut self) -> bool {
        if self.is_focus_layout() {
            if let Some(snap) = self.focus_snapshot.take() {
                self.chrome = snap.chrome;
                self.bottom_open = snap.bottom_open;
                self.bottom_hidden = snap.bottom_hidden;
                self.bottom_height = snap.bottom_height;
                self.last_open_height = snap.last_open_height;
                return true;
            }
            return false;
        }
        self.focus_snapshot = Some(ChromeSnapshot {
            chrome: self.chrome.clone(),
            bottom_open: self.bottom_open,
            bottom_hidden: self.bottom_hidden,
            bottom_height: self.bottom_height,
            last_open_height: self.last_open_height,
        });
        self.chrome.hide_left();
        self.chrome.hide_right();
        self.bottom_hidden = false;
        self.bottom_open = false;
        self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
        true
    }

    pub fn select_thread_id(&mut self, id: &str) -> bool {
        match self.threads.iter().position(|t| t.id == id) {
            Some(i) => {
                let _ = self.select(i);
                true
            }
            None => false,
        }
    }

    pub fn toggle_right_row(&mut self, id: impl Into<String>) {
        let id = id.into();
        if self.right_expanded_id.as_deref() == Some(id.as_str()) {
            self.right_expanded_id = None;
        } else {
            self.right_expanded_id = Some(id);
        }
    }

    pub fn collapse_right_row(&mut self) {
        self.right_expanded_id = None;
    }

    pub fn select_inspector(&mut self, tab: InspectorTab) -> bool {
        if self.inspector == tab {
            false
        } else {
            self.inspector = tab;
            self.right_expanded_id = None;
            true
        }
    }

    pub fn remember_mcp(&mut self, name: impl Into<String>) {
        self.selected_mcp = Some(name.into());
    }

    pub fn start_mcp(&mut self, name: &str) -> bool {
        match self.mcp.iter_mut().find(|m| m.name == name) {
            Some(row) if row.state == McpLife::Ready => false,
            Some(row) => {
                row.state = McpLife::Ready;
                true
            }
            None => false,
        }
    }

    pub fn stop_mcp(&mut self, name: &str) -> bool {
        match self.mcp.iter_mut().find(|m| m.name == name) {
            Some(row) => {
                row.state = McpLife::Stopped;
                true
            }
            None => false,
        }
    }

    pub fn select_file(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        if self.files.iter().any(|f| f == &path) {
            self.selected_file = Some(path);
            true
        } else {
            false
        }
    }

    pub fn toggle_file_expand(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        if !path.ends_with('/')
            && !self
                .files
                .iter()
                .any(|f| f == &path || f == &format!("{path}/"))
        {
            return false;
        }
        let key = if path.ends_with('/') {
            path
        } else {
            format!("{path}/")
        };
        if let Some(i) = self.file_expanded.iter().position(|p| p == &key) {
            self.file_expanded.remove(i);
        } else {
            self.file_expanded.push(key);
        }
        true
    }

    pub fn files_visible(&self) -> Vec<String> {
        let shown = crate::workbench::filter_files(&self.files, &self.file_filter);
        shown
            .into_iter()
            .filter(|path| ancestors_expanded(self, path))
            .collect()
    }

    pub fn apply_handshake(&mut self, hello_ok: bool, ping_ok: bool) {
        self.hello_ok = hello_ok;
        self.ping_ok = ping_ok;
        self.connection = crate::review::handshake_state(hello_ok, ping_ok);
    }

    pub fn select_diff(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        if self.diff_rows.iter().any(|r| r.path == path) {
            self.selected_diff = Some(path);
            true
        } else {
            false
        }
    }

    pub fn select_model(&mut self, model: impl Into<String>) -> bool {
        let model = model.into();
        if self.model == model {
            return false;
        }
        if self.models.iter().any(|m| m == &model) {
            self.model = model;
            true
        } else {
            false
        }
    }

    pub fn usage_lines(&self) -> String {
        format!(
            "Turns\n{}\n\nTokens\n{}\n\nNote\nlocal snapshot only",
            self.usage_turns, self.usage_tokens
        )
    }

    pub fn insert_file_mention(&mut self) -> bool {
        if self.selected_file.is_none() {
            if let Some(path) = self
                .right_expanded_id
                .as_deref()
                .and_then(|id| id.strip_prefix("file:"))
                .map(str::to_owned)
            {
                let _ = self.select_file(&path);
            } else if let Some(path) = self.selected_diff.clone() {
                self.selected_file = Some(path);
            }
        }
        let Some(path) = self.selected_file.clone() else {
            return false;
        };
        let mention = format!(" `@{path}` ");
        self.cursor = crate::composer::insert_at(&mut self.draft, self.cursor, &mention);
        true
    }

    pub fn push_notice(&mut self, kind: crate::notices::NoticeKind, text: impl Into<String>) {
        crate::notices::push_notice(&mut self.notices, &mut self.next_notice, kind, text);
    }

    pub fn agent_rows(&self) -> Vec<crate::agents::AgentRow> {
        self.threads
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.archived)
            .map(|(i, t)| crate::agents::AgentRow::from_thread(i, t, i == self.selected))
            .collect()
    }

    pub fn pop_out_inspector(&mut self) -> bool {
        if self.inspector_popped {
            return false;
        }
        match self.forest.detach(multiplexer_layout::PaneId(3)) {
            Ok(_) => {
                self.inspector_popped = true;
                self.chrome.hide_right();
                self.push_notice(
                    crate::notices::NoticeKind::Info,
                    "Inspector detached. Same HWND this wave. Dock with Ctrl+Shift+E.",
                );
                true
            }
            Err(_) => {
                self.push_notice(
                    crate::notices::NoticeKind::Warn,
                    "Cannot detach inspector (last primary pane).",
                );
                false
            }
        }
    }

    pub fn dock_inspector(&mut self) -> bool {
        if !self.inspector_popped {
            return false;
        }
        match self.forest.redock(multiplexer_layout::PaneId(3)) {
            Ok(_) => {
                self.inspector_popped = false;
                self.chrome.open_right();
                true
            }
            Err(_) => false,
        }
    }

    pub fn close_pop_out(&mut self) -> bool {
        self.dock_inspector()
    }

    pub fn next_region(&mut self) -> FocusRegion {
        self.focus_region = self.focus_region.next();
        self.focus_region
    }

    pub fn nudge_bottom(&mut self, delta: f32) -> bool {
        if self.bottom_hidden {
            return false;
        }
        let before = (self.bottom_height, self.bottom_open);
        let next = if self.bottom_open {
            self.bottom_height + delta
        } else {
            BOTTOM_HEIGHT_COLLAPSED + delta
        };
        self.set_bottom_height(next);
        (self.bottom_height, self.bottom_open) != before
    }

    pub fn pin_selected(&mut self) -> bool {
        match self.selected_thread_mut() {
            Some(t) => {
                t.pinned = !t.pinned;
                true
            }
            None => false,
        }
    }

    pub fn mark_selected_unread(&mut self) -> bool {
        match self.selected_thread_mut() {
            Some(t) => {
                t.unread = true;
                true
            }
            None => false,
        }
    }

    pub fn archive_selected(&mut self) -> bool {
        match self.selected_thread_mut() {
            Some(t) => {
                if t.archived {
                    false
                } else {
                    t.archived = true;
                    true
                }
            }
            None => false,
        }
    }

    pub fn apply_layout_persist(&mut self, snap: &crate::persist::LayoutPersist) {
        self.chrome.left = crate::persist::parse_rail(&snap.left);
        self.chrome.right = crate::persist::parse_rail(&snap.right);
        self.chrome.left_width = snap.left_width.clamp(LEFT_WIDTH_MIN, LEFT_WIDTH_MAX);
        self.chrome.right_width = snap.right_width.clamp(RIGHT_WIDTH_MIN, RIGHT_WIDTH_MAX);
        self.bottom_open = snap.bottom_open;
        self.bottom_hidden = snap.bottom_hidden;
        self.bottom_height = snap.bottom_height;
        if snap.inspector_popped && !self.inspector_popped {
            let _ = self.pop_out_inspector();
        }
    }

    pub fn restore_crash(&mut self, journal: &crate::persist::CrashJournal) -> bool {
        if !journal.marker || journal.threads.is_empty() {
            return false;
        }
        self.threads = journal
            .threads
            .iter()
            .map(|t| Thread {
                id: t.id.clone(),
                title: t.title.clone(),
                messages: t
                    .messages
                    .iter()
                    .map(|(role, text)| ChatMessage {
                        role: if role == "user" {
                            Role::User
                        } else {
                            Role::Assistant
                        },
                        text: text.clone(),
                    })
                    .collect(),
                status: t.status.clone(),
                model: t.model.clone(),
                pinned: false,
                unread: false,
                archived: false,
            })
            .collect();
        self.selected = 0;
        self.thread_drafts = journal.drafts.clone();
        self.restore_selected_draft();
        self.push_notice(
            crate::notices::NoticeKind::Info,
            crate::persist::crash_restore_notice(),
        );
        true
    }

    /// Branch for the title pill. Reminder label is not a branch name.
    pub fn branch_label(&self) -> String {
        for line in self.git_status.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                let name = rest.split("...").next().unwrap_or(rest).trim();
                if !name.is_empty() {
                    return name.to_owned();
                }
            }
            if let Some(rest) = t.strip_prefix("On branch ") {
                return rest.trim().to_owned();
            }
        }
        "no-branch".into()
    }

    pub fn reset_outlook_chrome(&mut self) {
        self.chrome = ChromeLayout::default();
        self.bottom_open = false;
        self.bottom_hidden = false;
        self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
        self.last_open_height = BOTTOM_HEIGHT_EXPANDED;
        self.left_section = LeftSection::Threads;
        self.right_expanded_id = None;
        self.focus_snapshot = None;
        if self.inspector_popped {
            let _ = self.dock_inspector();
        }
        self.forest = multiplexer_layout::LayoutForest::default_outlook();
        self.inspector_popped = false;
        self.focus_region = FocusRegion::Center;
        self.context_menu = None;
    }

    pub fn toggle_center_mode(&mut self) {
        self.center_mode = self.center_mode.toggle();
    }

    pub fn set_center_mode(&mut self, mode: crate::center::CenterMode) -> bool {
        if self.center_mode == mode {
            false
        } else {
            self.center_mode = mode;
            true
        }
    }

    pub fn set_diff_sort(&mut self, sort: crate::diff_view::DiffSort) -> bool {
        if self.diff_sort == sort {
            false
        } else {
            self.diff_sort = sort;
            true
        }
    }

    pub fn apply_porcelain(&mut self, text: &str) {
        let mut rows = crate::diff_view::parse_porcelain(text);
        crate::diff_view::mark_last_turn(&mut rows, &self.last_turn_paths);
        self.diff_rows = crate::diff_view::sort_diffs(rows, self.diff_sort);
    }

    pub fn remember_turn_paths(&mut self, paths: Vec<String>) {
        self.last_turn_paths = paths;
        crate::diff_view::mark_last_turn(&mut self.diff_rows, &self.last_turn_paths);
        self.diff_rows = crate::diff_view::sort_diffs(self.diff_rows.clone(), self.diff_sort);
    }

    pub fn visible_diffs(&self) -> Vec<crate::diff_view::DiffRow> {
        crate::diff_view::sort_diffs(self.diff_rows.clone(), self.diff_sort)
    }

    pub fn diff_detail(&self) -> String {
        if self.diff_rows.is_empty() {
            return "No working-tree diffs. Reload after a Grok turn. Text only, no apply.".into();
        }
        let mut out = format!("Sort {}\nText only, no apply.\n\n", self.diff_sort.label());
        for row in self.visible_diffs() {
            let star = if row.last_turn { "*" } else { " " };
            let sel = if self.selected_diff.as_deref() == Some(row.path.as_str()) {
                ">"
            } else {
                " "
            };
            out.push_str(&format!("{sel}{star} {}  {}\n", row.status, row.path));
        }
        if !self.diff_text.is_empty() {
            out.push_str("\nPreview\n");
            out.push_str(&self.diff_text);
        }
        out
    }

    pub fn browser_detail(&self) -> String {
        let url = if self.browser_url.is_empty() {
            "(no URL)"
        } else {
            self.browser_url.as_str()
        };
        format!("System browser only. CDP/HAR is later.\n\n{url}")
    }

    pub fn agents_detail(&self) -> String {
        let mut out = String::from("Local threads only. Subagent spawn is not wired.\n\n");
        for row in self.agent_rows() {
            out.push_str(&format!(
                "{}  [{}]  {}  {} msgs\n",
                row.title,
                row.status.as_str(),
                row.model,
                row.messages
            ));
        }
        out
    }

    pub fn toggle_core_reserved(&mut self, index: usize) -> bool {
        match self.cores.iter_mut().find(|c| c.index == index) {
            Some(c) => {
                c.reserved = !c.reserved;
                true
            }
            None => false,
        }
    }

    pub fn set_file_filter(&mut self, query: impl Into<String>) {
        self.file_filter = query.into();
    }
}

fn parse_skill_label(raw: &str) -> (String, String) {
    if let Some((name, rest)) = raw.rsplit_once('[') {
        let source = rest.trim().trim_end_matches(']').trim();
        (name.trim().to_owned(), source.to_owned())
    } else {
        (raw.to_owned(), "user".to_owned())
    }
}

fn ancestors_expanded(ws: &Workspace, path: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() <= 1 {
        return true;
    }
    let mut prefix = String::new();
    for part in parts.iter().take(parts.len() - 1) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        let dir = format!("{prefix}/");
        let listed = ws.files.iter().any(|f| f == &dir || f == &prefix);
        if listed {
            let open = ws
                .file_expanded
                .iter()
                .any(|e| e == &dir || e.trim_end_matches('/') == prefix);
            if !open {
                return false;
            }
        }
    }
    true
}

/// Tiny 8-tick bar. `usage` is a percent. NaN and non-finite values are empty.
fn tiny_usage_bar(usage: f32) -> String {
    let width = 8;
    let usage = if usage.is_finite() {
        usage.clamp(0.0, 100.0)
    } else {
        0.0
    };
    let filled = ((usage / 100.0) * width as f32).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_workspace_has_one_empty_thread() {
        let ws = Workspace::new("Multiplexer", "fake");
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.threads[0].title, "New chat");
        assert_eq!(ws.selected, 0);
        assert!(ws.draft.is_empty());
        assert_eq!(ws.inspector, InspectorTab::Session);
        assert!(ws.title_bar().contains("Multiplexer"));
        assert!(ws.title_bar().contains("fake"));
        assert!(ws.title_bar().contains("disconnected"));
        assert_eq!(ws.cursor, 0);
        assert_eq!(ws.models.as_slice(), ["fake"]);
        assert!(ws.files.is_empty());
        assert!(ws.skills.is_empty());
        assert!(ws.git_status.is_empty());
        assert!(ws.term_draft.is_empty());
        assert!(!ws.palette_open);
        assert!(!ws.help_open);
        assert!(ws.selected_checkpoint.is_none());
        assert!(ws.selected_worktree.is_none());
    }

    #[test]
    fn send_draft_creates_user_message_and_renames_thread() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.send_draft(), None);
        assert_eq!(ws.usage_turns, 0);
        ws.set_draft("  hello world  ");
        assert_eq!(ws.send_draft().as_deref(), Some("hello world"));
        let t = ws.selected_thread().unwrap();
        assert_eq!(t.title, "hello world");
        assert_eq!(t.messages.len(), 1);
        assert_eq!(t.messages[0].role, Role::User);
        assert_eq!(t.status, "running");
        assert!(ws.draft.is_empty());
        assert_eq!(ws.usage_turns, 1);
        assert!(ws.session_detail(None).contains("Turns\n1"));
        assert!(!ws.select_model("m"));
        assert!(!ws.select_model("nope"));
    }

    #[test]
    fn type_and_backspace_edit_draft() {
        let mut ws = Workspace::new("p", "m");
        ws.type_char('a');
        ws.type_char('\n');
        ws.type_char('b');
        assert_eq!(ws.draft, "ab");
        ws.backspace();
        assert_eq!(ws.draft, "a");
    }

    #[test]
    fn new_thread_selects_the_new_one() {
        let mut ws = Workspace::new("p", "m");
        ws.set_draft("x");
        ws.send_draft();
        let id = ws.new_thread();
        assert_eq!(id, "thr-2");
        assert_eq!(ws.selected, 1);
        assert_eq!(ws.threads.len(), 2);
        assert!(ws.draft.is_empty());
        assert!(ws.select(0));
        assert_eq!(ws.selected, 0);
        assert!(!ws.select(ws.threads.len()));
        assert_eq!(ws.selected, 0);
        assert!(!ws.select(9));
        assert_eq!(ws.selected, 0);
    }

    #[test]
    fn assistant_and_error_update_status() {
        let mut ws = Workspace::new("p", "m");
        ws.set_draft("q");
        ws.send_draft();
        ws.push_assistant("answer");
        assert_eq!(ws.selected_thread().unwrap().status, "idle");
        assert_eq!(
            ws.selected_thread().unwrap().messages[1].role,
            Role::Assistant
        );
        ws.mark_error("boom");
        assert_eq!(ws.selected_thread().unwrap().status, "error");
    }

    #[test]
    fn send_draft_without_thread_still_returns_text() {
        let mut ws = Workspace::new("p", "m");
        ws.threads.clear();
        ws.set_draft("orphan");
        assert_eq!(ws.send_draft().as_deref(), Some("orphan"));
        assert!(ws.draft.is_empty());
        ws.set_draft("later");
        ws.push_assistant("no thread");
        ws.mark_error("no thread");
        assert!(ws.selected_thread().is_none());
    }

    #[test]
    fn send_draft_does_not_rename_custom_title() {
        let mut ws = Workspace::new("p", "m");
        ws.set_draft("first title that is definitely longer than forty characters");
        ws.send_draft();
        assert_eq!(ws.selected_thread().unwrap().title.chars().count(), 40);
        ws.set_draft("second");
        ws.send_draft();
        assert_eq!(ws.selected_thread().unwrap().title.chars().count(), 40);
        assert_eq!(ws.selected_thread().unwrap().messages.len(), 2);
    }

    #[test]
    fn inspector_labels_and_connect() {
        assert_eq!(InspectorTab::Session.label(), "Session");
        assert_eq!(InspectorTab::Resources.label(), "Cores");
        assert_eq!(InspectorTab::Mcp.label(), "MCP");
        assert_eq!(InspectorTab::Checkpoints.label(), "Points");
        assert_eq!(InspectorTab::Git.label(), "Git");
        assert_eq!(InspectorTab::Terminal.label(), "Term");
        assert_eq!(InspectorTab::Skills.label(), "Skills");
        assert_eq!(InspectorTab::Files.label(), "Files");
        assert_eq!(InspectorTab::Activity.label(), "Activity");
        assert_eq!(InspectorTab::Agents.label(), "Agents");
        assert_eq!(InspectorTab::Diff.label(), "Diffs");
        assert_eq!(InspectorTab::Browser.label(), "Browser");
        assert_eq!(InspectorTab::all().len(), 12);
        let mut ws = Workspace::new("p", "m");
        ws.connect(vec!["sess-1".into()]);
        assert!(ws.connection.is_connected());
        assert_eq!(ws.connection.session_count(), 1);
        assert!(ws.title_bar().contains("connected"));
        assert_eq!(Workspace::thread_preview(&ws.threads[0]), "Empty thread");
        ws.set_draft("hello");
        ws.send_draft();
        assert!(Workspace::thread_preview(ws.selected_thread().unwrap()).starts_with("You:"));
        assert!(ws.session_detail(Some("sess-1")).contains("sess-1"));
        assert!(ws.resource_detail().contains("Worktrees"));
        assert!(ws.mcp_detail().contains("No MCP servers"));
        ws.mcp.push(McpRow {
            name: "linear".into(),
            command: "npx".into(),
            transport: "stdio".into(),
            state: McpLife::Stopped,
        });
        assert!(ws.mcp_detail().contains("linear"));
        ws.cores.push(CoreRow {
            index: 0,
            usage: 12.0,
            reserved: true,
        });
        assert!(ws.resource_detail().contains("cpu0"));
        ws.checkpoints.push(CheckpointRow {
            id: "cp-1".into(),
            label: "start".into(),
        });
        assert!(ws.checkpoint_detail().contains("cp-1"));
        ws.set_reminder("main", "C:/repo");
        assert_eq!(ws.reminder.as_ref().map(|r| r.0.as_str()), Some("main"));
        ws.dismiss_reminder();
        assert!(ws.reminder.is_none());
        ws.push_terminal("ready");
        assert_eq!(ws.terminal_log.last().map(String::as_str), Some("ready"));
        assert!(ws.busy);
        ws.mark_interrupted();
        assert!(!ws.busy);
    }

    #[test]
    fn chrome_default_is_open_with_roomy_rails() {
        let c = ChromeLayout::default();
        assert!(c.left_open() && c.right_open());
        assert!(c.left_width >= LEFT_WIDTH_MIN);
        assert!(c.right_width >= RIGHT_WIDTH_MIN);
        assert_eq!(c.occupied_left(), c.left_width);
        assert_eq!(c.occupied_right(), c.right_width);
    }

    #[test]
    fn chrome_toggle_hides_to_collapsed_strip() {
        let mut c = ChromeLayout::default();
        c.toggle_left();
        c.toggle_right();
        assert!(!c.left_open() && !c.right_open());
        assert_eq!(c.left, RailVis::IconRail);
        assert_eq!(c.occupied_left(), RAIL_COLLAPSED);
        assert_eq!(c.occupied_right(), RAIL_COLLAPSED);
        c.toggle_left();
        assert!(c.left_open());
        assert_eq!(c.occupied_left(), c.left_width);
        c.hide_left();
        c.hide_right();
        assert!(!c.left_open() && !c.right_open());
        assert_eq!(c.occupied_left(), 0.0);
        assert_eq!(c.occupied_right(), 0.0);
    }

    #[test]
    fn chrome_resize_clamps_and_reopens() {
        let mut c = ChromeLayout {
            left: RailVis::IconRail,
            ..ChromeLayout::default()
        };
        c.set_left_width(80.0);
        assert_eq!(c.left_width, LEFT_WIDTH_MIN);
        assert!(c.left_open());
        c.set_left_width(900.0);
        assert_eq!(c.left_width, LEFT_WIDTH_MAX);
        c.set_right_width(10.0);
        assert_eq!(c.right_width, RIGHT_WIDTH_MIN);
        c.nudge_right(40.0);
        assert_eq!(c.right_width, RIGHT_WIDTH_MIN + 40.0);
        c.nudge_left(-20.0);
        assert_eq!(c.left_width, LEFT_WIDTH_MAX - 20.0);
    }

    #[test]
    fn inspector_all_is_twelve_tabs() {
        let all = InspectorTab::all();
        assert_eq!(all.len(), 12);
        assert_eq!(
            all,
            [
                InspectorTab::Session,
                InspectorTab::Resources,
                InspectorTab::Mcp,
                InspectorTab::Checkpoints,
                InspectorTab::Git,
                InspectorTab::Terminal,
                InspectorTab::Skills,
                InspectorTab::Files,
                InspectorTab::Activity,
                InspectorTab::Agents,
                InspectorTab::Diff,
                InspectorTab::Browser,
            ]
        );
        assert_eq!(
            all.map(InspectorTab::label),
            [
                "Session", "Cores", "MCP", "Points", "Git", "Term", "Skills", "Files", "Activity",
                "Agents", "Diffs", "Browser"
            ]
        );
    }

    #[test]
    fn center_mode_and_diff_sort_work() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.center_mode, crate::center::CenterMode::Gui);
        ws.toggle_center_mode();
        assert_eq!(ws.center_mode, crate::center::CenterMode::GrokTui);
        assert!(!ws.set_center_mode(crate::center::CenterMode::GrokTui));
        assert!(ws.set_center_mode(crate::center::CenterMode::Gui));
        ws.apply_porcelain(" M zebra.rs\n M alpha.rs\n");
        ws.remember_turn_paths(vec!["zebra.rs".into()]);
        assert_eq!(ws.diff_sort, crate::diff_view::DiffSort::LastTurn);
        let first = ws.visible_diffs();
        assert_eq!(first[0].path, "zebra.rs");
        assert!(first[0].last_turn);
        assert!(ws.set_diff_sort(crate::diff_view::DiffSort::FileName));
        assert_eq!(ws.visible_diffs()[0].path, "alpha.rs");
        assert!(ws.diff_detail().contains("alpha.rs"));
        assert!(ws.browser_detail().contains("CDP/HAR"));
    }

    #[test]
    fn mcp_start_sets_ready_and_stop_releases() {
        let mut ws = Workspace::new("p", "m");
        assert!(!ws.start_mcp("linear"));
        ws.mcp.push(McpRow {
            name: "linear".into(),
            command: "npx".into(),
            transport: "stdio".into(),
            state: McpLife::Stopped,
        });
        assert!(ws.start_mcp("linear"));
        assert_eq!(ws.mcp[0].state, McpLife::Ready);
        assert!(ws.mcp_detail().contains("ready"));
        assert!(ws.stop_mcp("linear"));
        assert_eq!(ws.mcp[0].state, McpLife::Stopped);
        assert!(ws.mcp_detail().contains("stopped"));
    }

    #[test]
    fn file_tree_select_expand_and_mention() {
        let mut ws = Workspace::new("p", "m");
        ws.set_files(vec![
            "src/".into(),
            "src/lib.rs".into(),
            "Cargo.toml".into(),
        ]);
        assert!(ws.files_visible().contains(&"src/".into()));
        assert!(ws.files_visible().contains(&"Cargo.toml".into()));
        assert!(!ws.files_visible().contains(&"src/lib.rs".into()));
        assert!(ws.toggle_file_expand("src/"));
        assert!(ws.files_visible().contains(&"src/lib.rs".into()));
        assert!(ws.select_file("src/lib.rs"));
        ws.set_draft("see");
        assert!(ws.insert_file_mention());
        assert!(ws.draft.contains("`@src/lib.rs`"));
        assert!(ws.toggle_file_expand("src/"));
        assert!(!ws.files_visible().contains(&"src/lib.rs".into()));
    }

    #[test]
    fn branch_label_uses_git_status_else_no_branch() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.branch_label(), "no-branch");
        ws.set_reminder("existing", "C:/wt");
        assert_eq!(ws.branch_label(), "no-branch");
        ws.set_git_status("## feat/ui...origin/feat/ui\n");
        assert_eq!(ws.branch_label(), "feat/ui");
        ws.set_git_status("On branch main\n");
        assert_eq!(ws.branch_label(), "main");
    }

    #[test]
    fn reset_outlook_chrome_reopens_rails_and_collapses_bottom() {
        let mut ws = Workspace::new("p", "m");
        ws.chrome.left = RailVis::IconRail;
        ws.toggle_bottom();
        ws.select_left_section(LeftSection::Files);
        ws.reset_outlook_chrome();
        assert!(ws.chrome.left_open() && ws.chrome.right_open());
        assert!(!ws.bottom_open);
        assert!(!ws.bottom_hidden);
        assert_eq!(ws.bottom_height, BOTTOM_HEIGHT_COLLAPSED);
        assert_eq!(ws.left_section, LeftSection::Threads);
    }

    #[test]
    fn left_section_and_bottom_drawer() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.left_section, LeftSection::Threads);
        assert!(!ws.bottom_open);
        assert_eq!(ws.occupied_bottom(), BOTTOM_HEIGHT_COLLAPSED);
        assert!(!ws.select_left_section(LeftSection::Threads));
        assert!(ws.select_left_section(LeftSection::Files));
        assert_eq!(ws.left_section, LeftSection::Files);
        ws.toggle_bottom();
        assert!(ws.bottom_open);
        assert_eq!(ws.bottom_height, BOTTOM_HEIGHT_EXPANDED);
        ws.toggle_bottom();
        assert!(!ws.bottom_open);
        assert_eq!(ws.bottom_height, BOTTOM_HEIGHT_COLLAPSED);
        ws.set_bottom_height(200.0);
        assert!(ws.bottom_open);
        assert_eq!(ws.bottom_height, 200.0);
        ws.set_bottom_height(10.0);
        assert_eq!(ws.bottom_height, BOTTOM_HEIGHT_COLLAPSED);
        assert_eq!(BOTTOM_HEIGHT_OPEN_MIN, 80.0);
        assert!(!ws.bottom_open);
        assert_eq!(ws.occupied_bottom(), 36.0);
    }

    #[test]
    fn right_row_accordion_and_tab_clears() {
        let mut ws = Workspace::new("p", "m");
        ws.toggle_right_row("core:0");
        assert_eq!(ws.right_expanded_id.as_deref(), Some("core:0"));
        ws.toggle_right_row("core:1");
        assert_eq!(ws.right_expanded_id.as_deref(), Some("core:1"));
        ws.toggle_right_row("core:1");
        assert!(ws.right_expanded_id.is_none());
        ws.toggle_right_row("mcp:linear");
        assert!(ws.select_inspector(InspectorTab::Mcp));
        assert!(ws.right_expanded_id.is_none());
        assert!(!ws.select_inspector(InspectorTab::Mcp));
        ws.collapse_right_row();
        assert!(ws.right_expanded_id.is_none());
        assert_eq!(LeftSection::all().len(), 4);
        assert_eq!(LeftSection::Threads.rail_label(), "Chats");
        assert_eq!(LeftSection::Files.rail_label(), "Projects");
    }

    #[test]
    fn delete_thread_refuses_last() {
        let mut ws = Workspace::new("p", "m");
        let id = ws.threads[0].id.clone();
        assert_eq!(ws.threads.len(), 1);
        assert!(!ws.delete_thread(0));
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.threads[0].id, id);
        assert_eq!(ws.selected, 0);
        assert!(!ws.delete_thread(1));
        assert!(!ws.delete_thread(99));
        assert_eq!(ws.threads.len(), 1);
    }

    #[test]
    fn delete_thread_reselects() {
        let mut ws = Workspace::new("p", "m");
        ws.new_thread();
        ws.new_thread();
        assert_eq!(ws.threads.len(), 3);
        assert_eq!(ws.selected, 2);
        assert!(ws.rename_thread(1, "kept"));
        assert_eq!(ws.threads[1].title, "kept");
        assert!(!ws.rename_thread(9, "nope"));

        assert!(ws.delete_thread(2));
        assert_eq!(ws.threads.len(), 2);
        assert_eq!(ws.selected, 1);
        assert_eq!(ws.threads[1].title, "kept");

        let keep_id = ws.threads[1].id.clone();
        assert!(ws.delete_thread(0));
        assert_eq!(ws.threads.len(), 1);
        assert_eq!(ws.selected, 0);
        assert_eq!(ws.threads[0].id, keep_id);
        assert_eq!(ws.threads[0].title, "kept");

        ws.new_thread();
        ws.new_thread();
        assert!(ws.select(1));
        let prev_id = ws.threads[0].id.clone();
        assert!(ws.delete_thread(1));
        assert_eq!(ws.selected, 0);
        assert_eq!(ws.threads[0].id, prev_id);
        assert!(!ws.delete_thread(ws.threads.len()));
    }

    #[test]
    fn cycle_model_rotates() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.models.as_slice(), ["m"]);
        assert_eq!(ws.model, "m");
        assert_eq!(ws.cycle_model(), "m");
        assert_eq!(ws.model, "m");

        ws.set_models(vec!["alpha".into(), "beta".into(), "gamma".into()]);
        assert_eq!(ws.models.as_slice(), ["alpha", "beta", "gamma"]);
        assert_eq!(ws.model, "alpha");
        assert_eq!(ws.cycle_model(), "beta");
        assert_eq!(ws.model, "beta");
        assert_eq!(ws.cycle_model(), "gamma");
        assert_eq!(ws.cycle_model(), "alpha");
        assert_eq!(ws.model, "alpha");

        ws.set_models(Vec::new());
        assert_eq!(ws.models.as_slice(), ["alpha"]);
        assert_eq!(ws.model, "alpha");
        assert_eq!(ws.cycle_model(), "alpha");

        ws.set_models(vec!["alpha".into(), "delta".into()]);
        assert_eq!(ws.model, "alpha");
        assert_eq!(ws.cycle_model(), "delta");
    }

    #[test]
    fn cursor_insert_and_backspace_middle() {
        let mut ws = Workspace::new("p", "m");
        ws.type_char('a');
        ws.type_char('b');
        ws.type_char('c');
        assert_eq!(ws.draft, "abc");
        assert_eq!(ws.cursor, 3);

        ws.move_cursor_left();
        ws.move_cursor_left();
        assert_eq!(ws.cursor, 1);
        ws.type_char('X');
        assert_eq!(ws.draft, "aXbc");
        assert_eq!(ws.cursor, 2);

        ws.backspace();
        assert_eq!(ws.draft, "abc");
        assert_eq!(ws.cursor, 1);

        ws.move_cursor_home();
        assert_eq!(ws.cursor, 0);
        ws.backspace();
        assert_eq!(ws.draft, "abc");
        assert_eq!(ws.cursor, 0);

        ws.type_char('Z');
        assert_eq!(ws.draft, "Zabc");
        assert_eq!(ws.cursor, 1);

        ws.move_cursor_end();
        assert_eq!(ws.cursor, 4);
        ws.move_cursor_right();
        assert_eq!(ws.cursor, 4);
        ws.type_char('!');
        assert_eq!(ws.draft, "Zabc!");
        assert_eq!(ws.cursor, 5);

        ws.set_draft("éx");
        assert_eq!(ws.cursor, 2);
        ws.move_cursor_left();
        ws.backspace();
        assert_eq!(ws.draft, "x");
        assert_eq!(ws.cursor, 0);
    }

    #[test]
    fn palette_and_help_toggle() {
        let mut ws = Workspace::new("p", "m");
        assert!(!ws.palette_open);
        assert!(!ws.help_open);
        let closed = ws.session_detail(None);
        assert!(closed.contains("Palette\nclosed"));
        assert!(closed.contains("Help\nclosed"));
        assert!(closed.contains("Models\nm"));

        ws.toggle_palette();
        assert!(ws.palette_open);
        assert!(ws.session_detail(None).contains("Palette\nopen"));
        ws.toggle_palette();
        assert!(!ws.palette_open);
        ws.toggle_palette();
        ws.close_palette();
        assert!(!ws.palette_open);
        ws.close_palette();
        assert!(!ws.palette_open);

        ws.toggle_help();
        assert!(ws.help_open);
        assert!(!ws.palette_open);
        assert!(ws.session_detail(None).contains("Help\nopen"));
        ws.toggle_help();
        assert!(!ws.help_open);
    }

    #[test]
    fn take_term_draft_trims() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.take_term_draft(), None);
        assert!(ws.term_draft.is_empty());

        ws.set_term_draft("  ls -la  ");
        assert_eq!(ws.take_term_draft().as_deref(), Some("ls -la"));
        assert!(ws.term_draft.is_empty());

        ws.set_term_draft("   ");
        assert_eq!(ws.take_term_draft(), None);
        assert!(ws.term_draft.is_empty());

        ws.type_term_char('e');
        ws.type_term_char('c');
        ws.type_term_char('h');
        ws.type_term_char('o');
        ws.type_term_char('\n');
        assert_eq!(ws.term_draft, "echo");
        ws.backspace_term();
        assert_eq!(ws.term_draft, "ech");
        assert_eq!(ws.take_term_draft().as_deref(), Some("ech"));
        assert!(ws.term_draft.is_empty());
    }

    #[test]
    fn git_terminal_skills_detail_copy() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.skills_detail(), "No skills found under .grok/skills");
        ws.set_skills(vec!["review".into(), "tdd".into()]);
        let skills = ws.skills_detail();
        assert!(skills.contains("review"));
        assert!(skills.contains("tdd"));
        assert!(!skills.contains("No skills found under .grok/skills"));

        ws.worktrees.push("wt-a".into());
        ws.worktrees.push("wt-b".into());
        ws.selected_worktree = Some(1);
        ws.set_git_status("main · clean");
        let git = ws.git_detail();
        assert!(git.contains("wt-a"));
        assert!(git.contains("wt-b"));
        assert!(git.contains("main · clean"));
        assert!(git.contains("Worktrees"));
        assert!(git.contains("Status"));

        ws.push_terminal("ready");
        ws.push_terminal("compiled");
        ws.set_term_draft("git status");
        let term = ws.terminal_detail();
        assert!(term.contains("ready"));
        assert!(term.contains("compiled"));
        assert!(term.contains("git status"));

        ws.set_files(vec!["src/lib.rs".into()]);
        assert!(ws.resource_detail().contains("src/lib.rs"));
        assert!(ws.resource_detail().contains("Files"));

        ws.create_local_checkpoint("cp-local", "snap");
        assert!(ws.checkpoint_detail().contains("cp-local"));
        assert!(ws.checkpoint_detail().contains("snap"));
        ws.select_checkpoint(Some("cp-local".into()));
        assert_eq!(ws.selected_checkpoint.as_deref(), Some("cp-local"));
        assert!(ws.checkpoint_detail().contains("cp-local"));
        ws.select_checkpoint(None);
        assert!(ws.selected_checkpoint.is_none());
    }

    #[test]
    fn resource_detail_does_not_claim_job_object() {
        let ws = Workspace::new("p", "m");
        let detail = ws.resource_detail();
        assert!(!detail.contains("Job Object"));
        assert!(!detail.contains("armed"));
        assert!(!detail.contains("kill-on-close"));
        assert!(detail.contains("sample") || detail.contains("flag"));
    }

    #[test]
    fn connect_empty_sessions_is_ready_not_connected() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.connection.status_label(), "disconnected");
        assert!(!ws.connection.is_connected());
        ws.connect(Vec::new());
        assert_eq!(ws.connection.status_label(), "ready");
        assert!(!ws.connection.is_connected());
        assert!(!ws.title_bar().contains("connected"));
        ws.connect(vec!["sess-1".into()]);
        assert!(ws.connection.is_connected());
        assert!(ws.title_bar().contains("connected"));
    }

    #[test]
    fn hide_left_is_hidden_not_icon() {
        let mut c = ChromeLayout::default();
        c.hide_left();
        assert_eq!(c.left, RailVis::Hidden);
        assert_eq!(c.occupied_left(), 0.0);
        c.toggle_left();
        assert_eq!(c.left, RailVis::IconRail);
        c.toggle_left();
        assert_eq!(c.left, RailVis::Open);
    }

    #[test]
    fn bottom_hidden_occupies_zero_and_toggle_restores_last_open() {
        let mut ws = Workspace::new("p", "m");
        ws.set_bottom_height(200.0);
        assert_eq!(ws.bottom_height, 200.0);
        ws.toggle_bottom();
        assert!(!ws.bottom_open);
        assert_eq!(ws.occupied_bottom(), BOTTOM_HEIGHT_COLLAPSED);
        ws.toggle_bottom();
        assert_eq!(ws.bottom_height, 200.0);
        ws.hide_bottom();
        assert_eq!(ws.occupied_bottom(), 0.0);
        ws.toggle_bottom();
        assert!(!ws.bottom_hidden);
        assert!(!ws.bottom_open);
        assert_eq!(ws.occupied_bottom(), BOTTOM_HEIGHT_COLLAPSED);
    }

    #[test]
    fn focus_layout_hides_both_rails_and_second_call_restores() {
        let mut ws = Workspace::new("p", "m");
        ws.chrome.set_left_width(300.0);
        assert!(ws.focus_layout());
        assert_eq!(ws.chrome.occupied_left(), 0.0);
        assert_eq!(ws.chrome.occupied_right(), 0.0);
        assert!(ws.is_focus_layout());
        assert!(ws.focus_layout());
        assert_eq!(ws.chrome.left_width, 300.0);
        assert!(ws.chrome.left_open());
        assert!(!ws.is_focus_layout());
    }

    #[test]
    fn checkpoint_detail_says_pointer_only() {
        let ws = Workspace::new("p", "m");
        let detail = ws.checkpoint_detail();
        assert!(detail.contains("Pointer only"));
        assert!(detail.contains("Files unchanged") || detail.contains("not snapshotted"));
        assert!(!detail.contains("session runtime"));
        let mut git = Workspace::new("p", "m");
        git.git_checkpoints = true;
        git.checkpoints.push(crate::workspace::CheckpointRow {
            id: "cp-1".into(),
            label: "start".into(),
        });
        let hidden = git.checkpoint_detail();
        assert!(hidden.contains("Hidden git refs"));
        assert!(hidden.contains("restores"));
        assert!(!hidden.contains("Pointer only"));
    }

    #[test]
    fn agent_rows_are_threads_not_session_ids() {
        let mut ws = Workspace::new("p", "m");
        let before = ws.agent_rows().len();
        ws.connect(vec!["sess-1".into()]);
        assert_eq!(ws.agent_rows().len(), before);
        assert_eq!(ws.agent_rows()[0].id, ws.threads[0].id);
        assert!(ws.select_thread_id(&ws.threads[0].id.clone()));
    }

    #[test]
    fn skills_hooks_core_toggle_and_file_filter() {
        let mut ws = Workspace::new("p", "m");
        ws.set_skills(vec!["review [project]".into(), "tdd".into()]);
        assert_eq!(ws.skill_items.len(), 2);
        assert_eq!(ws.skill_items[0].source, "project");
        assert!(ws.toggle_skill("review"));
        assert!(!ws.skill_items[0].enabled);
        assert!(
            ws.skills_detail().contains("not loaded into grok")
                || ws.skills_detail().contains("local flag")
        );
        ws.hooks.push(("pre".into(), "commit".into()));
        assert!(ws.skills_detail().contains("Hooks"));
        ws.cores.push(crate::workspace::CoreRow {
            index: 3,
            usage: 1.0,
            reserved: false,
        });
        assert!(ws.toggle_core_reserved(3));
        assert!(ws.cores.iter().any(|c| c.index == 3 && c.reserved));
        ws.set_file_filter("lib");
        ws.set_files(vec!["src/lib.rs".into(), "src/main.rs".into()]);
        assert_eq!(ws.files_visible(), vec!["src/lib.rs".to_owned()]);
        ws.remember_mcp("linear");
        assert_eq!(ws.selected_mcp.as_deref(), Some("linear"));
        assert_eq!(ws.threads[0].model, "m");
        ws.mark_error("boom");
        assert!(ws.session_detail(None).contains("boom"));
        assert!(ws.resource_detail().contains("RAM"));
        assert!(ws.agents_detail().contains("m"));
    }

    #[test]
    fn handshake_and_diff_select_and_mcp_ready_is_noop() {
        let mut ws = Workspace::new("C:/repo", "m");
        ws.apply_handshake(true, true);
        assert_eq!(ws.connection, crate::ConnectionState::Ready);
        assert!(ws.session_detail(None).contains("hello ok"));
        assert!(ws.session_detail(None).contains("local snapshot only"));
        ws.apply_handshake(true, false);
        assert_eq!(ws.connection, crate::ConnectionState::Connecting);
        ws.set_git_status("## feat...origin/feat [ahead 1]\n M a.rs\n");
        let git = ws.git_detail();
        assert!(git.contains("feat"));
        assert!(git.contains("dirty"));
        assert!(git.contains("Create"));
        ws.apply_porcelain(" M a.rs\n");
        assert!(ws.select_diff("a.rs"));
        assert_eq!(ws.selected_diff.as_deref(), Some("a.rs"));
        assert!(!ws.select_diff("missing.rs"));
        ws.diff_text = "diff --git a/a.rs".into();
        assert!(ws.diff_detail().contains("Preview"));
        assert!(ws.resource_detail().contains("No contained processes"));
        ws.mcp.push(crate::workspace::McpRow {
            name: "x".into(),
            command: "npx".into(),
            transport: "stdio".into(),
            state: crate::workspace::McpLife::Ready,
        });
        assert!(!ws.start_mcp("x"));
        ws.file_filter = "nope".into();
        ws.set_files(vec!["src/a.rs".into()]);
        assert!(ws.files_visible().is_empty());
    }

    #[test]
    fn per_thread_draft_stashes_and_restores() {
        let mut ws = Workspace::new("p", "m");
        ws.set_draft("hello");
        ws.new_thread();
        assert!(ws.draft.is_empty());
        ws.set_draft("second");
        assert!(ws.select(0));
        assert_eq!(ws.draft, "hello");
        assert!(ws.select(1));
        assert_eq!(ws.draft, "second");
        assert!(!ws.select(1));
        assert_eq!(ws.draft, "second");
    }

    #[test]
    fn overlay_policy_and_recent_commands() {
        let mut ws = Workspace::new("p", "m");
        ws.open_overlay(crate::overlay::OverlayKind::Palette);
        ws.open_overlay(crate::overlay::OverlayKind::Help);
        assert!(ws.palette_open && ws.help_open);
        ws.open_overlay(crate::overlay::OverlayKind::Settings);
        assert!(ws.settings_open && !ws.palette_open && !ws.help_open);
        assert_eq!(
            ws.pop_overlay(),
            Some(crate::overlay::OverlayKind::Settings)
        );
        assert!(!ws.settings_open);
        ws.search_query = "foo".into();
        ws.search_selected = 2;
        ws.open_overlay(crate::overlay::OverlayKind::Search);
        ws.close_overlay(crate::overlay::OverlayKind::Search);
        assert!(ws.search_query.is_empty());
        assert_eq!(ws.search_selected, 0);

        ws.remember_command("new-chat");
        ws.remember_command("mcp");
        ws.remember_command("new-chat");
        assert_eq!(ws.recent_commands, vec!["new-chat", "mcp"]);
        ws.remember_command("");
        assert_eq!(ws.recent_commands.len(), 2);
        for i in 0..10 {
            ws.remember_command(&format!("c{i}"));
        }
        assert_eq!(ws.recent_commands.len(), 8);
        assert_eq!(ws.recent_commands[0], "c9");
        assert!(!ws.dismiss_newest_notice());
        ws.push_notice(crate::notices::NoticeKind::Info, "hi");
        assert!(ws.dismiss_newest_notice());
        assert!(ws.notices.is_empty());
    }

    #[test]
    fn pop_out_inspector_detaches_pane_three_and_hides_right() {
        let mut ws = Workspace::new("p", "m");
        assert!(!ws.inspector_popped);
        assert!(ws.pop_out_inspector());
        assert!(ws.inspector_popped);
        assert_eq!(ws.chrome.occupied_right(), 0.0);
        assert_eq!(ws.forest.window_count(), 2);
        assert!(!ws.pop_out_inspector());
        assert!(ws.dock_inspector());
        assert!(!ws.inspector_popped);
        assert!(ws.chrome.right_open());
        assert_eq!(ws.forest.window_count(), 1);
        assert!(!ws.close_pop_out());
    }

    #[test]
    fn next_region_and_nudge_bottom() {
        let mut ws = Workspace::new("p", "m");
        assert_eq!(ws.focus_region, FocusRegion::Center);
        assert_eq!(ws.next_region(), FocusRegion::Right);
        assert_eq!(ws.next_region(), FocusRegion::Bottom);
        assert_eq!(ws.next_region(), FocusRegion::Left);
        assert_eq!(ws.next_region(), FocusRegion::Center);
        assert_eq!(FocusRegion::Left.label(), "left");
        assert!(!ws.nudge_bottom(16.0));
        ws.toggle_bottom();
        assert!(ws.nudge_bottom(16.0));
        assert_eq!(ws.bottom_height, BOTTOM_HEIGHT_EXPANDED + 16.0);
        ws.hide_bottom();
        assert!(!ws.nudge_bottom(16.0));
    }

    #[test]
    fn pin_unread_archive_and_crash_restore() {
        let mut ws = Workspace::new("p", "m");
        assert!(ws.pin_selected());
        assert!(ws.threads[0].pinned);
        assert!(ws.mark_selected_unread());
        assert!(ws.threads[0].unread);
        assert!(ws.archive_selected());
        assert!(ws.threads[0].archived);
        assert!(ws.agent_rows().is_empty());
        assert!(!ws.archive_selected());
        ws.threads.clear();
        assert!(!ws.pin_selected());
        assert!(!ws.mark_selected_unread());
        assert!(!ws.archive_selected());

        let mut src = Workspace::new("p", "m");
        src.set_draft("restored");
        src.send_draft();
        let j = crate::persist::journal_from_workspace(&src);
        let mut dest = Workspace::new("p", "m");
        assert!(dest.restore_crash(&j));
        assert!(dest.threads.iter().any(|t| t.title == "restored"));
        assert!(dest.notices.iter().any(|n| n.text.contains("not replayed")));
        let empty = crate::persist::CrashJournal::default();
        assert!(!dest.restore_crash(&empty));
    }
}
