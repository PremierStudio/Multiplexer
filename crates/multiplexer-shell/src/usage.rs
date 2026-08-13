//! Local session usage snapshot (not billing).

/// Turns and tokens for the current local session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageSnapshot {
    pub turns: u64,
    pub tokens: u64,
    pub note: String,
}

impl UsageSnapshot {
    /// Empty local-account snapshot.
    pub fn local() -> Self {
        Self {
            turns: 0,
            tokens: 0,
            note: "local snapshot only".into(),
        }
    }

    /// Record one finished turn and add its token count.
    pub fn bump_turn(&mut self, tokens: u64) {
        self.turns += 1;
        self.tokens += tokens;
    }

    /// Session-detail block: Turns, Tokens, and the account note.
    pub fn format_lines(&self) -> String {
        format!(
            "Turns: {}\nTokens: {}\n{}",
            self.turns, self.tokens, self.note
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_snapshot_formats_session_detail() {
        let mut snap = UsageSnapshot::local();
        assert_eq!(snap.turns, 0);
        assert_eq!(snap.tokens, 0);
        assert_eq!(snap.note, "local snapshot only");

        snap.bump_turn(10);
        snap.bump_turn(20);
        assert_eq!(snap.turns, 2);
        assert_eq!(snap.tokens, 30);

        let text = snap.format_lines();
        assert!(text.contains("Turns"), "session detail must name Turns");
        assert!(text.contains("Tokens"), "session detail must name Tokens");
        assert!(
            text.contains("local snapshot only"),
            "session detail must include the note"
        );
        assert!(text.contains("Turns: 2"), "{text}");
        assert!(text.contains("Tokens: 30"), "{text}");
        assert!(
            !text.contains("n/a"),
            "local snapshot prints counts, not n/a"
        );
    }
}
