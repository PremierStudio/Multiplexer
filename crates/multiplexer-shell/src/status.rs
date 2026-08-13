//! Compact client status snapshot for the chrome status strip.

use crate::workspace::Workspace;

/// Counts and busy flag projected from a [`Workspace`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientStatus {
    pub busy: bool,
    pub session_id: Option<String>,
    pub thread_count: usize,
    pub mcp_count: usize,
    pub core_count: usize,
    pub checkpoint_count: usize,
    pub worktree_count: usize,
    pub palette_open: bool,
    pub help_open: bool,
    pub term_lines: usize,
}

fn overlay_flags(ws: &Workspace) -> (bool, bool) {
    (ws.palette_open, ws.help_open)
}

/// Copy counts and busy flag from `ws`. `session_id` is taken as given.
pub fn status_from(ws: &Workspace, session_id: Option<String>) -> ClientStatus {
    let (palette_open, help_open) = overlay_flags(ws);
    ClientStatus {
        busy: ws.busy,
        session_id,
        thread_count: ws.threads.len(),
        mcp_count: ws.mcp.len(),
        core_count: ws.cores.len(),
        checkpoint_count: ws.checkpoints.len(),
        worktree_count: ws.worktrees.len(),
        palette_open,
        help_open,
        term_lines: ws.terminal_log.len(),
    }
}

/// Compact strip: `idle · 3 chats · 12 mcp · 16 cores · 1 cp · 0 wt`.
pub fn status_line(s: &ClientStatus) -> String {
    let mode = if s.busy { "running" } else { "idle" };
    let mut line = format!(
        "{} · {} chats · {} mcp · {} cores · {} cp · {} wt",
        mode, s.thread_count, s.mcp_count, s.core_count, s.checkpoint_count, s.worktree_count,
    );
    if s.palette_open {
        line = format!("palette · {line}");
    }
    if s.help_open {
        line = format!("help · {line}");
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{CheckpointRow, CoreRow, McpRow, Workspace};

    fn line_status(
        busy: bool,
        threads: usize,
        mcp: usize,
        cores: usize,
        cps: usize,
    ) -> ClientStatus {
        ClientStatus {
            busy,
            session_id: None,
            thread_count: threads,
            mcp_count: mcp,
            core_count: cores,
            checkpoint_count: cps,
            worktree_count: 0,
            palette_open: false,
            help_open: false,
            term_lines: 0,
        }
    }

    #[test]
    fn status_from_new_workspace_is_idle_one_thread() {
        let ws = Workspace::new("p", "m");
        let s = status_from(&ws, None);
        assert_eq!(
            s,
            ClientStatus {
                busy: false,
                session_id: None,
                thread_count: 1,
                mcp_count: 0,
                core_count: 0,
                checkpoint_count: 0,
                worktree_count: 0,
                palette_open: false,
                help_open: false,
                term_lines: 0,
            }
        );
        assert_eq!(
            status_line(&s),
            "idle · 1 chats · 0 mcp · 0 cores · 0 cp · 0 wt"
        );
    }

    #[test]
    fn status_from_passes_session_id_through_not_connection() {
        let mut ws = Workspace::new("p", "m");
        ws.connect(vec!["from-connection".into()]);
        let some = status_from(&ws, Some("explicit".into()));
        assert_eq!(some.session_id.as_deref(), Some("explicit"));
        assert_ne!(some.session_id.as_deref(), Some("from-connection"));
        let none = status_from(&ws, None);
        assert_eq!(none.session_id, None);
    }

    #[test]
    fn status_from_copies_inventory_counts() {
        let mut ws = Workspace::new("p", "m");
        ws.new_thread();
        ws.new_thread();
        for i in 0..12 {
            ws.mcp.push(McpRow {
                name: format!("m{i}"),
                command: "npx".into(),
                transport: "stdio".into(),
                state: crate::workspace::McpLife::Stopped,
            });
        }
        for i in 0..16 {
            ws.cores.push(CoreRow {
                index: i,
                usage: 0.0,
                reserved: false,
            });
        }
        ws.checkpoints.push(CheckpointRow {
            id: "cp-1".into(),
            label: "start".into(),
        });
        let s = status_from(&ws, Some("sess-1".into()));
        assert!(!s.busy);
        assert_eq!(s.session_id.as_deref(), Some("sess-1"));
        assert_eq!(s.thread_count, 3);
        assert_eq!(s.mcp_count, 12);
        assert_eq!(s.core_count, 16);
        assert_eq!(s.checkpoint_count, 1);
        assert_eq!(
            status_line(&s),
            "idle · 3 chats · 12 mcp · 16 cores · 1 cp · 0 wt"
        );
    }

    #[test]
    fn status_from_uses_workspace_busy_not_thread_status() {
        let mut ws = Workspace::new("p", "m");
        ws.threads.clear();
        ws.set_draft("go");
        ws.send_draft();
        assert!(ws.busy);
        let running = status_from(&ws, None);
        assert!(running.busy);
        assert_eq!(running.thread_count, 0);
        assert_eq!(
            status_line(&running),
            "running · 0 chats · 0 mcp · 0 cores · 0 cp · 0 wt"
        );

        let mut ws = Workspace::new("p", "m");
        ws.set_draft("go");
        ws.send_draft();
        ws.mark_error("boom");
        assert!(!ws.busy);
        assert_eq!(ws.selected_thread().unwrap().status, "error");
        let idle = status_from(&ws, None);
        assert!(!idle.busy);
        assert!(status_line(&idle).starts_with("idle ·"));
        assert!(!status_line(&idle).starts_with("running ·"));
        assert!(!status_line(&idle).starts_with("error ·"));
    }

    #[test]
    fn status_line_matches_spec_idle_and_running() {
        let idle = line_status(false, 3, 12, 16, 1);
        assert_eq!(
            status_line(&idle),
            "idle · 3 chats · 12 mcp · 16 cores · 1 cp · 0 wt"
        );
        let running = ClientStatus {
            busy: true,
            session_id: Some("sess".into()),
            ..idle
        };
        assert_eq!(
            status_line(&running),
            "running · 3 chats · 12 mcp · 16 cores · 1 cp · 0 wt"
        );
        assert!(
            !status_line(&running).contains("sess"),
            "session_id is stored, not printed"
        );
    }

    #[test]
    fn status_line_keeps_fixed_labels_and_field_order() {
        let s = line_status(false, 1, 1, 1, 1);
        assert_eq!(
            status_line(&s),
            "idle · 1 chats · 1 mcp · 1 cores · 1 cp · 0 wt"
        );
        let s = line_status(true, 2, 4, 8, 0);
        let line = status_line(&s);
        assert_eq!(line, "running · 2 chats · 4 mcp · 8 cores · 0 cp · 0 wt");
        assert!(line.contains(" chats · "));
        assert!(line.contains(" mcp · "));
        assert!(line.contains(" cores · "));
        assert!(line.contains(" cp"));
        assert!(line.ends_with(" wt"));
        assert!(!line.contains("chat ·"));
        assert!(!line.contains("core ·"));
        assert!(!line.contains("cps"));
        assert_eq!(line.matches(" · ").count(), 5);
    }

    #[test]
    fn status_includes_worktrees() {
        let mut ws = Workspace::new("p", "m");
        ws.worktrees.push("wt-a".into());
        ws.worktrees.push("wt-b".into());
        ws.push_terminal("ready");
        ws.push_terminal("ok");
        let s = status_from(&ws, None);
        assert_eq!(s.worktree_count, 2);
        assert_eq!(s.term_lines, 2);
        assert_eq!(
            status_line(&s),
            "idle · 1 chats · 0 mcp · 0 cores · 0 cp · 2 wt"
        );
        assert!(status_line(&s).contains(" · 2 wt"));
    }

    #[test]
    fn palette_prefix() {
        let mut s = line_status(false, 1, 0, 0, 0);
        s.palette_open = true;
        assert_eq!(
            status_line(&s),
            "palette · idle · 1 chats · 0 mcp · 0 cores · 0 cp · 0 wt"
        );
        s.busy = true;
        assert!(status_line(&s).starts_with("palette · running ·"));
    }

    #[test]
    fn help_prefix_before_idle() {
        let mut s = line_status(false, 1, 0, 0, 0);
        s.help_open = true;
        let line = status_line(&s);
        assert!(line.starts_with("help · idle"));
        assert_eq!(
            line,
            "help · idle · 1 chats · 0 mcp · 0 cores · 0 cp · 0 wt"
        );
        s.palette_open = true;
        let both = status_line(&s);
        assert!(both.starts_with("help · palette · idle"));
    }
}
