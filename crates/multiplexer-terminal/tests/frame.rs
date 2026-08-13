//! Property tests for the last-frame VT buffer.

use multiplexer_terminal::PtyFrame;
use proptest::prelude::*;

proptest! {
    #[test]
    fn display_never_contains_esc(raw in "\\PC{0,80}") {
        let mut frame = PtyFrame::new(24, 8);
        frame.feed(&raw);
        let shown = frame.display();
        let esc = '\u{1b}';
        prop_assert!(!shown.contains(esc));
    }

    #[test]
    fn char_at_a_time_matches_whole(raw in "[a-zA-Z0-9 \\n\\r\\t]{0,60}") {
        let mut whole = PtyFrame::new(16, 6);
        let mut parts = PtyFrame::new(16, 6);
        whole.feed(&raw);
        for ch in raw.chars() {
            parts.feed(&ch.to_string());
        }
        prop_assert_eq!(whole.display(), parts.display());
        prop_assert_eq!(whole.cursor(), parts.cursor());
        prop_assert_eq!(whole.has_pending(), parts.has_pending());
    }

    #[test]
    fn split_csi_color_equals_whole(prefix in "[A-Za-z]{0,8}", suffix in "[A-Za-z]{0,8}") {
        let seq = format!("{prefix}\u{1b}[32m{suffix}\u{1b}[0m!");
        let mut whole = PtyFrame::new(40, 3);
        whole.feed(&seq);
        let mut parts = PtyFrame::new(40, 3);
        let bytes: Vec<char> = seq.chars().collect();
        let mid = bytes.len() / 2;
        let a: String = bytes[..mid].iter().collect();
        let b: String = bytes[mid..].iter().collect();
        parts.feed(&a);
        parts.feed(&b);
        prop_assert_eq!(whole.display(), parts.display());
        prop_assert_eq!(whole.display(), format!("{prefix}{suffix}!"));
    }

    #[test]
    fn cr_overwrite_keeps_suffix_then_rest_of_prefix(
        prefix in "[a-z]{1,12}",
        suffix in "[A-Z]{1,12}"
    ) {
        let mut frame = PtyFrame::new(32, 2);
        frame.feed(&format!("{prefix}\r{suffix}"));
        let line = frame.display();
        let prefix_chars: Vec<char> = prefix.chars().collect();
        let suffix_chars: Vec<char> = suffix.chars().collect();
        let mut expected: Vec<char> = suffix_chars.clone();
        if prefix_chars.len() > suffix_chars.len() {
            expected.extend(prefix_chars.into_iter().skip(suffix_chars.len()));
        }
        let expected: String = expected.into_iter().collect();
        prop_assert!(!line.contains('\r'));
        prop_assert_eq!(line, expected);
    }

    #[test]
    fn clear_then_text_drops_prior(prior in "[a-z]{1,16}", next in "[A-Z]{1,16}") {
        let mut frame = PtyFrame::new(40, 4);
        frame.feed(&prior);
        frame.feed(&format!("\u{1b}[2J\u{1b}[H{next}"));
        prop_assert_eq!(frame.display(), next);
        prop_assert!(!frame.display().contains(&prior));
    }
}
