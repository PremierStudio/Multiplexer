//! Typed agent rows over local threads. Spawn stays unwired.

use crate::workspace::Thread;

/// Pulse on a local thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadStatus {
    Idle,
    Running,
    Error,
}

impl ThreadStatus {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "running" | "busy" | "working" => Self::Running,
            "error" | "failed" | "fail" => Self::Error,
            _ => Self::Idle,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Error => "error",
        }
    }

    pub fn label(self) -> &'static str {
        self.as_str()
    }
}

/// One Agents-list row. Same vec as left Threads, different projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRow {
    pub index: usize,
    pub id: String,
    pub title: String,
    pub status: ThreadStatus,
    pub model: String,
    pub messages: usize,
    pub selected: bool,
    pub pinned: bool,
    pub unread: bool,
}

impl AgentRow {
    pub fn from_thread(index: usize, thread: &Thread, selected: bool) -> Self {
        Self {
            index,
            id: thread.id.clone(),
            title: crate::persist::thread_leaf_title(&thread.title, &thread.id),
            status: ThreadStatus::parse(&thread.status),
            model: thread.model.clone(),
            messages: thread.messages.len(),
            selected,
            pinned: thread.pinned,
            unread: thread.unread,
        }
    }
}

/// Honest `orchestration.list` stub. Subagents stay empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchestrationList {
    pub threads: usize,
    pub subagents: Vec<String>,
    pub note: String,
}

pub fn orchestration_list(thread_count: usize) -> OrchestrationList {
    OrchestrationList {
        threads: thread_count,
        subagents: Vec::new(),
        note: "Local threads only. Subagent spawn is not wired.".into(),
    }
}

pub fn orchestration_list_json(list: &OrchestrationList) -> String {
    serde_json::json!({
        "threads": list.threads,
        "subagents": list.subagents,
        "note": list.note,
    })
    .to_string()
}

pub fn orchestration_spawn_missing() -> &'static str {
    "method not found: orchestration.spawn"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Workspace;

    #[test]
    fn status_parse_is_narrow() {
        assert_eq!(ThreadStatus::parse("idle"), ThreadStatus::Idle);
        assert_eq!(ThreadStatus::parse("running"), ThreadStatus::Running);
        assert_eq!(ThreadStatus::parse("BUSY"), ThreadStatus::Running);
        assert_eq!(ThreadStatus::parse("error"), ThreadStatus::Error);
        assert_eq!(ThreadStatus::parse("failed"), ThreadStatus::Error);
        assert_eq!(ThreadStatus::parse("nope"), ThreadStatus::Idle);
        assert_eq!(ThreadStatus::parse(""), ThreadStatus::Idle);
        assert_ne!(ThreadStatus::Idle, ThreadStatus::Running);
        assert_eq!(ThreadStatus::Running.as_str(), "running");
        assert_eq!(ThreadStatus::Error.label(), "error");
    }

    #[test]
    fn agent_row_uses_leaf_title_not_raw_id() {
        let ws = Workspace::new("p", "m");
        let row = AgentRow::from_thread(0, &ws.threads[0], true);
        assert_eq!(row.title, "New chat");
        assert_ne!(row.title, row.id);
        assert!(!row.title.starts_with("thr-"));
        assert_eq!(row.status, ThreadStatus::Idle);
        assert_eq!(row.model, "m");
        assert!(row.selected);
        assert_eq!(row.messages, 0);
        assert!(!row.pinned);
        assert!(!row.unread);
    }

    #[test]
    fn agent_rows_are_threads_not_session_ids() {
        let mut ws = Workspace::new("p", "m");
        let before = ws.agent_rows().len();
        ws.connect(vec!["sess-1".into()]);
        assert_eq!(ws.agent_rows().len(), before);
        assert_eq!(ws.agent_rows()[0].id, ws.threads[0].id);
        assert_ne!(ws.agent_rows()[0].id, "sess-1");
    }

    #[test]
    fn orchestration_list_is_empty_subagents() {
        let list = orchestration_list(3);
        assert_eq!(list.threads, 3);
        assert!(list.subagents.is_empty());
        assert!(list.note.contains("not wired"));
        let raw = orchestration_list_json(&list);
        assert!(raw.contains("\"subagents\":[]"));
        assert!(raw.contains("\"threads\":3"));
        assert!(!raw.contains("spawned"));
        assert_eq!(
            orchestration_spawn_missing(),
            "method not found: orchestration.spawn"
        );
        assert_ne!(orchestration_list(0).threads, list.threads);
    }
}
