//! Pure desktop chrome for Phase 0.4.
//!
//! No GPUI types live here. The desktop binary projects [`DesktopChrome`]
//! into a window. Tests and CI stay headless.

use multiplexer_layout::{LayoutForest, LayoutNode, PaneId};

/// Title of the primary Multiplexer window.
pub const DEFAULT_WINDOW_TITLE: &str = "Multiplexer";

/// OS-window chrome: title plus the pane forest to project.
#[derive(Debug, Clone, PartialEq)]
pub struct DesktopChrome {
    pub title: String,
    pub layout: LayoutForest,
}

impl DesktopChrome {
    /// Outlook-style three-pane chrome for the primary window.
    pub fn default_outlook() -> Self {
        Self {
            title: DEFAULT_WINDOW_TITLE.to_owned(),
            layout: LayoutForest::default_outlook(),
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
}
