use multiplexer_layout::{LayoutForest, PaneId};
use multiplexer_shell::{DesktopChrome, DEFAULT_WINDOW_TITLE};

#[test]
fn shell_chrome_title_is_multiplexer() {
    let chrome = DesktopChrome::default_outlook();
    assert_eq!(DEFAULT_WINDOW_TITLE, "Multiplexer");
    assert_eq!(chrome.title, "Multiplexer");
    assert_eq!(chrome.title, DEFAULT_WINDOW_TITLE);
}

#[test]
fn default_layout_has_three_panes() {
    let chrome = DesktopChrome::default_outlook();
    assert_eq!(chrome.layout, LayoutForest::default_outlook());
    assert_eq!(chrome.live_pane_count(), 3);
    assert!(chrome.layout.contains_pane(PaneId(1)));
    assert!(chrome.layout.contains_pane(PaneId(2)));
    assert!(chrome.layout.contains_pane(PaneId(3)));
}
