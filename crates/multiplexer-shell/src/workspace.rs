//! Headless workspace model: threads, transcript, composer, inspector.

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
}

impl InspectorTab {
    pub fn label(self) -> &'static str {
        match self {
            Self::Session => "Session",
            Self::Resources => "Cores",
            Self::Mcp => "MCP",
        }
    }
}

/// Min/max pixel widths for the Outlook rails.
pub const LEFT_WIDTH_MIN: f32 = 180.0;
pub const LEFT_WIDTH_MAX: f32 = 420.0;
pub const RIGHT_WIDTH_MIN: f32 = 220.0;
pub const RIGHT_WIDTH_MAX: f32 = 480.0;
pub const RAIL_COLLAPSED: f32 = 36.0;

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
    next_id: u64,
}

impl Workspace {
    pub fn new(project: impl Into<String>, model: impl Into<String>) -> Self {
        let mut ws = Self {
            project: project.into(),
            model: model.into(),
            connection: ConnectionState::Disconnected,
            threads: Vec::new(),
            selected: 0,
            draft: String::new(),
            inspector: InspectorTab::Session,
            worktrees: Vec::new(),
            chrome: ChromeLayout::default(),
            next_id: 1,
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

    pub fn set_draft(&mut self, text: impl Into<String>) {
        self.draft = text.into();
    }

    pub fn type_char(&mut self, c: char) {
        if !c.is_control() {
            self.draft.push(c);
        }
    }

    pub fn backspace(&mut self) {
        self.draft.pop();
    }

    /// Take the draft as a user message. Returns the text if it was non-empty.
    pub fn send_draft(&mut self) -> Option<String> {
        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return None;
        }
        self.draft.clear();
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
    }

    pub fn mark_error(&mut self, message: impl Into<String>) {
        if let Some(thread) = self.selected_thread_mut() {
            thread.status = "error".to_owned();
            thread.messages.push(ChatMessage {
                role: Role::Assistant,
                text: message.into(),
            });
        }
    }

    pub fn connect(&mut self, session_ids: Vec<String>) {
        self.connection = ConnectionState::Connected { session_ids };
    }
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
        let mut ws = Workspace::new("p", "m");
        ws.connect(vec!["sess-1".into()]);
        assert!(ws.connection.is_connected());
        assert_eq!(ws.connection.session_count(), 1);
        assert!(ws.title_bar().contains("connected"));
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
}
