//! Toast stack. Desktop paints these; Workspace owns the list.

/// Visual kind of a toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Good,
    Warn,
    Danger,
}

/// One toast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub id: u64,
    pub kind: NoticeKind,
    pub text: String,
}

/// Stored cap. Oldest drop first.
pub const NOTICE_CAP: usize = 8;
/// Painted at once (newest).
pub const NOTICE_PAINT: usize = 3;

/// Info and Good auto-dismiss after this many milliseconds.
pub const NOTICE_AUTO_MS: u64 = 4000;

/// Push onto `notices`, assign `next_id`, cap at [`NOTICE_CAP`] (drop oldest).
pub fn push_notice(
    notices: &mut Vec<Notice>,
    next_id: &mut u64,
    kind: NoticeKind,
    text: impl Into<String>,
) -> u64 {
    let id = *next_id;
    *next_id += 1;
    notices.push(Notice {
        id,
        kind,
        text: text.into(),
    });
    if notices.len() > NOTICE_CAP {
        let drop = notices.len() - NOTICE_CAP;
        notices.drain(0..drop);
    }
    id
}

pub fn dismiss_notice(notices: &mut Vec<Notice>, id: u64) -> bool {
    let before = notices.len();
    notices.retain(|n| n.id != id);
    notices.len() != before
}

/// Newest toast. Used by Esc when no overlay is open.
pub fn dismiss_newest(notices: &mut Vec<Notice>) -> bool {
    notices.pop().is_some()
}

/// Last [`NOTICE_PAINT`] toasts (newest last).
pub fn visible_notices(notices: &[Notice]) -> &[Notice] {
    let start = notices.len().saturating_sub(NOTICE_PAINT);
    &notices[start..]
}

/// Info and Good fade; Warn and Danger stay.
pub fn auto_dismisses(kind: NoticeKind) -> bool {
    matches!(kind, NoticeKind::Info | NoticeKind::Good)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_at_eight_and_dismisses() {
        let mut notices = Vec::new();
        let mut next = 1;
        for i in 0..9 {
            push_notice(&mut notices, &mut next, NoticeKind::Info, format!("n{i}"));
        }
        assert_eq!(notices.len(), NOTICE_CAP);
        assert_eq!(notices[0].text, "n1");
        assert_eq!(notices[7].text, "n8");
        assert!(dismiss_notice(&mut notices, 3));
        assert_eq!(notices.len(), 7);
        assert!(!dismiss_notice(&mut notices, 3));
        assert_ne!(notices[0].kind, NoticeKind::Danger);
        assert_eq!(NOTICE_CAP, 8);
        assert_eq!(NOTICE_PAINT, 3);
        assert_eq!(NOTICE_AUTO_MS, 4000);
    }

    #[test]
    fn visible_is_last_three() {
        let mut notices = Vec::new();
        let mut next = 1;
        assert!(visible_notices(&notices).is_empty());
        for i in 0..5 {
            push_notice(&mut notices, &mut next, NoticeKind::Warn, format!("n{i}"));
        }
        let vis = visible_notices(&notices);
        assert_eq!(vis.len(), 3);
        assert_eq!(vis[0].text, "n2");
        assert_eq!(vis[2].text, "n4");
        assert!(dismiss_newest(&mut notices));
        assert_eq!(notices.last().map(|n| n.text.as_str()), Some("n3"));
        notices.clear();
        assert!(!dismiss_newest(&mut notices));
    }

    #[test]
    fn auto_dismiss_kinds() {
        assert!(auto_dismisses(NoticeKind::Info));
        assert!(auto_dismisses(NoticeKind::Good));
        assert!(!auto_dismisses(NoticeKind::Warn));
        assert!(!auto_dismisses(NoticeKind::Danger));
    }
}
