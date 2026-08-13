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

const NOTICE_CAP: usize = 5;

/// Push onto `notices`, assign `next_id`, cap at 5 (drop oldest).
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_caps_at_five_and_dismisses() {
        let mut notices = Vec::new();
        let mut next = 1;
        for i in 0..6 {
            push_notice(&mut notices, &mut next, NoticeKind::Info, format!("n{i}"));
        }
        assert_eq!(notices.len(), 5);
        assert_eq!(notices[0].text, "n1");
        assert_eq!(notices[4].text, "n5");
        assert!(dismiss_notice(&mut notices, 3));
        assert_eq!(notices.len(), 4);
        assert!(!dismiss_notice(&mut notices, 3));
        assert_ne!(notices[0].kind, NoticeKind::Danger);
    }
}
