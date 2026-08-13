//! Context menus for thread, file, MCP, and diff rows.

use crate::ClientAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Thread,
    File,
    Mcp,
    Diff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    pub id: &'static str,
    pub label: &'static str,
    pub action: ClientAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenMenu {
    pub kind: MenuKind,
    pub target: String,
    pub items: Vec<MenuItem>,
}

pub fn menu_for(kind: MenuKind, target: impl Into<String>) -> OpenMenu {
    OpenMenu {
        kind,
        target: target.into(),
        items: items(kind),
    }
}

pub fn items(kind: MenuKind) -> Vec<MenuItem> {
    match kind {
        MenuKind::Thread => vec![
            item("open", "Open", ClientAction::SelectThread(0)),
            item("pin", "Pin", ClientAction::PinThread),
            item("unread", "Mark unread", ClientAction::MarkUnread),
            item("copy-id", "Copy id", ClientAction::CopyThreadId),
            item("archive", "Archive", ClientAction::ArchiveThread),
            item("delete", "Delete", ClientAction::DeleteThread),
            item("stop", "Stop", ClientAction::Interrupt),
        ],
        MenuKind::File => vec![
            item("mention", "Mention", ClientAction::InsertFileMention),
            item("reveal", "Reveal", ClientAction::RevealFile),
            item("open", "Open external", ClientAction::OpenExternal),
        ],
        MenuKind::Mcp => vec![
            item("start", "Start flag", ClientAction::StartMcp),
            item("stop", "Stop flag", ClientAction::StopMcp),
            item("mention", "Mention", ClientAction::MentionMcp),
            item("reload", "Reload", ClientAction::RefreshMcp),
        ],
        MenuKind::Diff => vec![
            item("mention", "Mention", ClientAction::InsertFileMention),
            item("open", "Open external", ClientAction::OpenExternal),
        ],
    }
}

fn item(id: &'static str, label: &'static str, action: ClientAction) -> MenuItem {
    MenuItem { id, label, action }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_menu_has_open_pin_delete() {
        let m = menu_for(MenuKind::Thread, "thr-1");
        assert_eq!(m.target, "thr-1");
        let ids: Vec<_> = m.items.iter().map(|i| i.id).collect();
        assert!(ids.contains(&"open"));
        assert!(ids.contains(&"pin"));
        assert!(ids.contains(&"delete"));
        assert!(ids.contains(&"copy-id"));
        assert!(!ids.contains(&"apply"));
        assert_eq!(m.kind, MenuKind::Thread);
    }

    #[test]
    fn file_mcp_diff_menus_are_distinct() {
        let f = items(MenuKind::File);
        let m = items(MenuKind::Mcp);
        let d = items(MenuKind::Diff);
        assert!(f.iter().any(|i| i.id == "reveal"));
        assert!(m.iter().any(|i| i.id == "start"));
        assert!(d.iter().any(|i| i.label == "Open external"));
        assert_ne!(f.len(), m.len());
        assert!(m.len() > d.len());
        assert!(f.iter().all(|i| !i.label.is_empty()));
    }
}
