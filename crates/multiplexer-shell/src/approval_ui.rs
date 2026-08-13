//! Headless pending-approval card for the desktop chrome.

use crate::Workspace;

/// One tool-approval prompt waiting on the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingApproval {
    pub request_id: String,
    pub tool: String,
    pub session_id: String,
    pub summary: String,
}

impl PendingApproval {
    pub fn allow_label(&self) -> &'static str {
        "Allow"
    }

    pub fn deny_label(&self) -> &'static str {
        "Deny"
    }

    pub fn card_title(&self) -> String {
        format!("Allow {}?", self.tool)
    }

    pub fn card_body(&self) -> String {
        if self.summary.is_empty() {
            "Agent wants to run this tool.".to_owned()
        } else {
            self.summary.clone()
        }
    }
}

impl Workspace {
    pub fn set_pending_approval(&mut self, a: PendingApproval) {
        self.pending = Some(a);
    }

    pub fn clear_pending_approval(&mut self) {
        self.pending = None;
    }

    pub fn pending_approval(&self) -> Option<&PendingApproval> {
        self.pending.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(request_id: &str, tool: &str, session_id: &str, summary: &str) -> PendingApproval {
        PendingApproval {
            request_id: request_id.to_owned(),
            tool: tool.to_owned(),
            session_id: session_id.to_owned(),
            summary: summary.to_owned(),
        }
    }

    #[test]
    fn new_workspace_has_no_pending_approval() {
        let ws = Workspace::new("p", "m");
        assert!(ws.pending_approval().is_none());
        assert!(ws.pending.is_none());
    }

    #[test]
    fn set_pending_approval_stores_every_field() {
        let mut ws = Workspace::new("p", "m");
        let a = sample("req-1", "shell", "sess-1", "run ls");
        ws.set_pending_approval(a.clone());
        let got = ws.pending_approval().expect("pending approval");
        assert_eq!(got.request_id, "req-1");
        assert_eq!(got.tool, "shell");
        assert_eq!(got.session_id, "sess-1");
        assert_eq!(got.summary, "run ls");
        assert_eq!(got, &a);
        assert_eq!(ws.pending.as_ref(), Some(&a));
    }

    #[test]
    fn set_pending_approval_replaces_previous() {
        let mut ws = Workspace::new("p", "m");
        ws.set_pending_approval(sample("req-1", "shell", "sess-1", "run ls"));
        ws.set_pending_approval(sample("req-2", "fs_write", "sess-2", "overwrite README"));
        let got = ws.pending_approval().expect("replaced approval");
        assert_eq!(got.request_id, "req-2");
        assert_eq!(got.tool, "fs_write");
        assert_eq!(got.session_id, "sess-2");
        assert_eq!(got.summary, "overwrite README");
        assert_ne!(got.request_id, "req-1");
        assert_ne!(got.tool, "shell");
        assert_ne!(got.session_id, "sess-1");
        assert_ne!(got.summary, "run ls");
    }

    #[test]
    fn clear_pending_approval_drops_request() {
        let mut ws = Workspace::new("p", "m");
        ws.set_pending_approval(sample("req-1", "shell", "sess-1", "run ls"));
        assert!(ws.pending_approval().is_some());
        ws.clear_pending_approval();
        assert!(ws.pending_approval().is_none());
        assert!(ws.pending.is_none());
    }

    #[test]
    fn clear_pending_approval_is_idempotent() {
        let mut ws = Workspace::new("p", "m");
        ws.clear_pending_approval();
        ws.clear_pending_approval();
        assert!(ws.pending_approval().is_none());
        assert!(ws.pending.is_none());
    }

    #[test]
    fn card_copy_uses_tool_and_summary() {
        let a = sample("req-9", "fs_write", "sess-9", "overwrite README.md");
        assert_eq!(a.allow_label(), "Allow");
        assert_eq!(a.deny_label(), "Deny");
        assert_ne!(a.allow_label(), a.deny_label());
        assert_eq!(a.card_title(), "Allow fs_write?");
        assert_ne!(a.card_title(), "Allow shell?");
        assert_eq!(a.card_body(), "overwrite README.md");
        assert_eq!(a.card_body(), a.summary);
        assert_ne!(a.card_body(), "Agent wants to run this tool.");
        assert_ne!(a.card_body(), a.tool);
    }

    #[test]
    fn empty_summary_fallback() {
        let a = sample("req-0", "shell", "sess-0", "");
        assert!(a.summary.is_empty());
        assert_eq!(a.card_title(), "Allow shell?");
        assert_eq!(a.card_body(), "Agent wants to run this tool.");
        assert_ne!(a.card_body(), a.summary);
        assert_ne!(a.card_body(), a.tool);
        let filled = sample("req-1", "shell", "sess-0", "echo hi");
        assert_eq!(filled.card_body(), "echo hi");
        assert_ne!(filled.card_body(), "Agent wants to run this tool.");
    }
}
