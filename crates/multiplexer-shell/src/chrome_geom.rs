//! Pure layout math for title overflow and bottom-rail drag.

/// Title-bar controls dropped when `avail_px` is too narrow.
///
/// Drop order: remotes, then turns, then branch. Run, palette, and inspector stay.
pub fn title_overflow(avail_px: f32) -> Vec<&'static str> {
    let mut drop = Vec::new();
    if avail_px < 1100.0 {
        drop.push("remotes_pill");
    }
    if avail_px < 1000.0 {
        drop.push("turns_pill");
    }
    if avail_px < 900.0 {
        drop.push("branch_pill");
    }
    drop
}

/// Bottom drawer height from a mouse Y in the client viewport.
pub fn bottom_height_from_mouse(win_h: f32, mouse_y: f32, status_h: f32, handle_h: f32) -> f32 {
    win_h - mouse_y - status_h - handle_h
}

/// Remotes title pill. Detect only, never "connected".
pub fn remotes_pill_label(tailscale_detected: bool) -> &'static str {
    if tailscale_detected {
        "ts detected"
    } else {
        "local"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_overflow_drops_remotes_then_turns_then_branch() {
        assert!(title_overflow(1400.0).is_empty());
        assert!(title_overflow(1100.0).is_empty());
        assert_eq!(title_overflow(1099.0), vec!["remotes_pill"]);
        assert_eq!(title_overflow(1050.0), vec!["remotes_pill"]);
        assert!(title_overflow(1000.0).contains(&"remotes_pill"));
        assert!(!title_overflow(1000.0).contains(&"turns_pill"));
        assert_eq!(title_overflow(950.0), vec!["remotes_pill", "turns_pill"]);
        assert!(!title_overflow(900.0).contains(&"branch_pill"));
        assert_eq!(
            title_overflow(800.0),
            vec!["remotes_pill", "turns_pill", "branch_pill"]
        );
        assert!(!title_overflow(800.0).contains(&"run"));
        assert!(!title_overflow(800.0).contains(&"command_palette"));
        assert!(!title_overflow(800.0).contains(&"inspector_toggle"));
    }

    #[test]
    fn bottom_drag_subtracts_status_and_handle() {
        assert_eq!(bottom_height_from_mouse(1080.0, 800.0, 28.0, 8.0), 244.0);
        assert!(bottom_height_from_mouse(800.0, 400.0, 28.0, 8.0) < 800.0 - 400.0);
    }

    #[test]
    fn remotes_pill_is_detect_not_connected() {
        assert_eq!(remotes_pill_label(false), "local");
        assert_eq!(remotes_pill_label(true), "ts detected");
        assert_ne!(remotes_pill_label(true), "local+ts");
        assert!(!remotes_pill_label(true).contains("connected"));
    }
}
