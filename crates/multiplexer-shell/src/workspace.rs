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
        }
    }

    pub fn all() -> [InspectorTab; 10] {
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

/// Min/max pixel widths for the Outlook rails.
pub const LEFT_WIDTH_MIN: f32 = 180.0;
pub const LEFT_WIDTH_MAX: f32 = 420.0;
pub const RIGHT_WIDTH_MIN: f32 = 220.0;
pub const RIGHT_WIDTH_MAX: f32 = 480.0;
pub const RAIL_COLLAPSED: f32 = 44.0;
pub const BOTTOM_HEIGHT_COLLAPSED: f32 = 120.0;
pub const BOTTOM_HEIGHT_EXPANDED: f32 = 280.0;
pub const BOTTOM_HEIGHT_MIN: f32 = 120.0;
pub const BOTTOM_HEIGHT_MAX: f32 = 420.0;

/// Show/hide and width of the left and right rails.
#[derive(Debug, Clone, PartialEq)]
pub struct ChromeLayout {
    pub left_open: bool,
    pub right_open: bool,
    pub left_width: f32,
    pub right_width: f32,
}

impl Default for ChromeLayout {
    fn default() -> Self {
        Self {
            left_open: true,
            right_open: true,
            left_width: 248.0,
            right_width: 300.0,
        }
    }
}

impl ChromeLayout {
    pub fn toggle_left(&mut self) {
        self.left_open = !self.left_open;
    }

    pub fn toggle_right(&mut self) {
        self.right_open = !self.right_open;
    }

    pub fn set_left_width(&mut self, width: f32) {
        self.left_width = width.clamp(LEFT_WIDTH_MIN, LEFT_WIDTH_MAX);
        self.left_open = true;
    }

    pub fn set_right_width(&mut self, width: f32) {
        self.right_width = width.clamp(RIGHT_WIDTH_MIN, RIGHT_WIDTH_MAX);
        self.right_open = true;
    }

    pub fn nudge_left(&mut self, delta: f32) {
        self.set_left_width(self.left_width + delta);
    }

    pub fn nudge_right(&mut self, delta: f32) {
        self.set_right_width(self.right_width + delta);
    }

    /// Width the left rail occupies, including the collapsed strip.
    pub fn occupied_left(&self) -> f32 {
        if self.left_open {
            self.left_width
        } else {
            RAIL_COLLAPSED
        }
    }

    pub fn occupied_right(&self) -> f32 {
        if self.right_open {
            self.right_width
        } else {
            RAIL_COLLAPSED
        }
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
    pub bottom_height: f32,
    pub selected_file: Option<String>,
    pub notices: Vec<crate::notices::Notice>,
    pub settings: crate::settings::UiSettings,
    pub wt_path: String,
    pub wt_branch: String,
    pub wt_create_branch: bool,
    pub settings_open: bool,
    next_id: u64,
    next_notice: u64,
}

impl Workspace {
    pub fn new(project: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let mut ws = Self {
            project: project.into(),
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
            bottom_height: BOTTOM_HEIGHT_COLLAPSED,
            selected_file: None,
            notices: Vec::new(),
            settings: crate::settings::UiSettings::default(),
            wt_path: "../mux-feat".into(),
            wt_branch: "feat".into(),
            wt_create_branch: true,
            settings_open: false,
            next_id: 1,
            next_notice: 1,
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
        let id = format!("thr-{}", self.next_id);
        self.next_id += 1;
        self.threads.push(Thread {
            id: id.clone(),
            title: "New chat".to_owned(),
            messages: Vec::new(),
            status: "idle".to_owned(),
        });
        self.selected = self.threads.len() - 1;
        self.draft.clear();
        self.cursor = 0;
        id
    }

    pub fn select(&mut self, index: usize) -> bool {
        if index < self.threads.len() {
            self.selected = index;
            true
        } else {
            false
        }
    }

    /// Remove a thread. Keeps at least one. If the selected thread is
    /// removed, the previous index (or 0) becomes selected.
    pub fn delete_thread(&mut self, index: usize) -> bool {
        if self.threads.len() <= 1 || index >= self.threads.len() {
            return false;
        }
        self.threads.remove(index);
        if index < self.selected {
            self.selected -= 1;
        } else if index == self.selected {
            self.selected = self.selected.saturating_sub(1);
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
        if let Some(thread) = self.selected_thread_mut() {
            thread.status = "error".to_owned();
            thread.messages.push(ChatMessage {
                role: Role::Assistant,
                text: message.into(),
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
        self.connection = ConnectionState::Connected { session_ids };
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
        self.skills = skills;
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

    pub fn toggle_palette(&mut self) {
        self.palette_open = !self.palette_open;
    }

    pub fn close_palette(&mut self) {
        self.palette_open = false;
    }

    pub fn toggle_help(&mut self) {
        self.help_open = !self.help_open;
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
            "Project\n{}\n\nModel\n{}\n\nConnection\n{}\n\nSession\n{}\n\nThreads\n{}\n\nModels\n{}\n\nPalette\n{}\n\nHelp\n{}",
            self.project,
            self.model,
            self.connection.status_label(),
            session_id.unwrap_or("(none yet)"),
            self.threads.len(),
            models,
            if self.palette_open { "open" } else { "closed" },
            if self.help_open { "open" } else { "closed" },
        )
    }

    pub fn resource_detail(&self) -> String {
        let mut out =
            String::from("Reserved cores: 0, 1 (app)\nJob Object kill-on-close is armed.\n\n");
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
            return "No MCP servers in ~/.grok/config.toml\nReuse/teardown still applies when they start.".to_owned();
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
        if self.checkpoints.is_empty() {
            return "No checkpoints yet. A start checkpoint is created with the session runtime."
                .to_owned();
        }
        self.checkpoints
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
            .join("\n")
    }

    pub fn git_detail(&self) -> String {
        let mut out = String::from("Worktrees\n");
        if self.worktrees.is_empty() {
            out.push_str("(none listed)");
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
        if self.skills.is_empty() {
            "No skills found under .grok/skills".to_owned()
        } else {
            self.skills.join("\n")
        }
    }

    pub fn files_detail(&self) -> String {
        if self.files.is_empty() {
            "No project files listed.".to_owned()
        } else {
            self.files.join("\n")
        }
    }

    pub fn activity_detail(&self) -> String {
        if self.terminal_log.is_empty() {
            "No activity yet.".to_owned()
        } else {
            self.terminal_log.join("\n")
        }
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
        if self.bottom_open {
            self.bottom_open = false;
            self.bottom_height = BOTTOM_HEIGHT_COLLAPSED;
        } else {
            self.bottom_open = true;
            self.bottom_height = BOTTOM_HEIGHT_EXPANDED;
        }
    }

    pub fn set_bottom_height(&mut self, height: f32) {
        self.bottom_height = height.clamp(BOTTOM_HEIGHT_MIN, BOTTOM_HEIGHT_MAX);
        self.bottom_open = self.bottom_height > BOTTOM_HEIGHT_COLLAPSED + 0.5;
    }

    pub fn occupied_bottom(&self) -> f32 {
        self.bottom_height
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

    pub fn start_mcp(&mut self, name: &str) -> bool {
        match self.mcp.iter_mut().find(|m| m.name == name) {
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

    pub fn insert_file_mention(&mut self) -> bool {
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

    pub fn agent_rows(&self) -> Vec<(String, String, String, usize)> {
        self.threads
            .iter()
            .map(|t| {
                (
                    t.id.clone(),
                    t.title.clone(),
                    t.status.clone(),
                    t.messages.len(),
                )
            })
            .collect()
    }

    pub fn agents_detail(&self) -> String {
        let mut out = String::from("Local threads only. Subagent spawn is not wired.\n\n");
        for (id, title, status, n) in self.agent_rows() {
            out.push_str(&format!("{title}  [{status}]  {id}  {n} msgs\n"));
        }
        out
    }
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
        ws.set_draft("  hello world  ");
        assert_eq!(ws.send_draft().as_deref(), Some("hello world"));
        let t = ws.selected_thread().unwrap();
        assert_eq!(t.title, "hello world");
        assert_eq!(t.messages.len(), 1);
        assert_eq!(t.messages[0].role, Role::User);
        assert_eq!(t.status, "running");
        assert!(ws.draft.is_empty());
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
        assert_eq!(InspectorTab::all().len(), 10);
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
        assert!(c.left_open && c.right_open);
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
        assert!(!c.left_open && !c.right_open);
        assert_eq!(c.occupied_left(), RAIL_COLLAPSED);
        assert_eq!(c.occupied_right(), RAIL_COLLAPSED);
        c.toggle_left();
        assert!(c.left_open);
        assert_eq!(c.occupied_left(), c.left_width);
    }

    #[test]
    fn chrome_resize_clamps_and_reopens() {
        let mut c = ChromeLayout::default();
        c.left_open = false;
        c.set_left_width(80.0);
        assert_eq!(c.left_width, LEFT_WIDTH_MIN);
        assert!(c.left_open);
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
    fn inspector_all_is_ten_tabs() {
        let all = InspectorTab::all();
        assert_eq!(all.len(), 10);
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
            ]
        );
        assert_eq!(
            all.map(InspectorTab::label),
            [
                "Session", "Cores", "MCP", "Points", "Git", "Term", "Skills", "Files", "Activity",
                "Agents"
            ]
        );
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
        ws.set_files(vec!["src/lib.rs".into(), "Cargo.toml".into()]);
        assert!(!ws.select_file("missing.rs"));
        assert!(ws.select_file("src/lib.rs"));
        assert_eq!(ws.selected_file.as_deref(), Some("src/lib.rs"));
        ws.set_draft("see");
        assert!(ws.insert_file_mention());
        assert!(ws.draft.contains("`@src/lib.rs`"));
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
        assert_eq!(ws.bottom_height, BOTTOM_HEIGHT_MIN);
        assert!(!ws.bottom_open);
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
}
