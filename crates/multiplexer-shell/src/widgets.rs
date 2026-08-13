//! Headless UI specs. Desktop projects these into GPUI.

pub const HEIGHT_COMPACT: u16 = 32;
pub const HEIGHT_ROW: u16 = 36;
pub const HEIGHT_COMFORT: u16 = 44;
pub const HEIGHT_TITLE: u16 = 48;
pub const HEIGHT_RAIL: u16 = 48;
pub const HEIGHT_CARD: u16 = 56;

/// Visual treatment of a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Ghost,
    Danger,
    Icon,
}

/// One clickable control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonSpec {
    pub kind: ButtonKind,
    pub label: String,
    pub hint: String,
    pub icon: String,
    pub enabled: bool,
    pub busy: bool,
}

impl ButtonSpec {
    pub fn ghost(label: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            kind: ButtonKind::Ghost,
            label: label.into(),
            hint: hint.into(),
            icon: String::new(),
            enabled: true,
            busy: false,
        }
    }

    pub fn primary(label: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            kind: ButtonKind::Primary,
            label: label.into(),
            hint: hint.into(),
            icon: String::new(),
            enabled: true,
            busy: false,
        }
    }

    pub fn icon(icon: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            kind: ButtonKind::Icon,
            label: String::new(),
            hint: hint.into(),
            icon: icon.into(),
            enabled: true,
            busy: false,
        }
    }

    pub fn height(self) -> u16 {
        match self.kind {
            ButtonKind::Icon | ButtonKind::Ghost => HEIGHT_COMPACT,
            ButtonKind::Primary | ButtonKind::Danger => HEIGHT_COMFORT,
        }
    }
}

/// Tone for badges and pills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Good,
    Warn,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadgeSpec {
    pub tone: Tone,
    pub text: String,
}

impl BadgeSpec {
    pub fn new(tone: Tone, text: impl Into<String>) -> Self {
        Self {
            tone,
            text: text.into(),
        }
    }
}

/// One list row that can expand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRowSpec {
    pub id: String,
    pub icon: String,
    pub title: String,
    pub subtitle: String,
    pub meta: String,
    pub badge: Option<BadgeSpec>,
    pub selected: bool,
    pub busy: bool,
    pub expandable: bool,
    pub expanded: bool,
    pub children_count: usize,
}

impl ListRowSpec {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: String::new(),
            title: title.into(),
            subtitle: String::new(),
            meta: String::new(),
            badge: None,
            selected: false,
            busy: false,
            expandable: true,
            expanded: false,
            children_count: 0,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    pub fn with_meta(mut self, meta: impl Into<String>) -> Self {
        self.meta = meta.into();
        self
    }

    pub fn with_badge(mut self, badge: BadgeSpec) -> Self {
        self.badge = Some(badge);
        self
    }

    pub fn height(&self) -> u16 {
        HEIGHT_CARD
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabSpec {
    pub id: String,
    pub icon: String,
    pub label: String,
    pub selected: bool,
    pub count: usize,
}

impl TabSpec {
    pub fn height() -> u16 {
        HEIGHT_ROW
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyStateSpec {
    pub title: String,
    pub body: String,
    pub action: String,
}

impl EmptyStateSpec {
    pub fn chat() -> Self {
        Self {
            title: "What should we build?".into(),
            body: "Send a grok -p turn, or switch to TUI for a system shell.".into(),
            action: "New chat".into(),
        }
    }

    pub fn tiles() -> [&'static str; 4] {
        empty_state_tiles()
    }
}

/// Suggestion tiles on the empty center.
pub fn empty_state_tiles() -> [&'static str; 4] {
    [
        "What can you do?",
        "Summarize this repo",
        "git status",
        "Run the tests",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_row_is_not_equal_idle() {
        let idle = ListRowSpec::new("a", "Alpha");
        let mut on = idle.clone();
        on.selected = true;
        assert_ne!(idle, on);
        assert_eq!(idle.height(), HEIGHT_CARD);
        assert_eq!(
            ListRowSpec {
                expanded: true,
                ..idle.clone()
            }
            .height(),
            HEIGHT_CARD
        );
    }

    #[test]
    fn badge_tone_changes_label() {
        let n = BadgeSpec::new(Tone::Neutral, "idle");
        let g = BadgeSpec::new(Tone::Good, "ready");
        assert_ne!(n.tone, g.tone);
        assert_ne!(n.text, g.text);
        assert_eq!(g.tone, Tone::Good);
    }

    #[test]
    fn empty_state_has_action() {
        let e = EmptyStateSpec::chat();
        assert!(!e.action.is_empty());
        assert!(!e.title.is_empty());
        assert_eq!(empty_state_tiles().len(), 4);
        assert_eq!(ButtonSpec::icon("⌘", "palette").height(), HEIGHT_COMPACT);
        assert_eq!(
            ButtonSpec::primary("Send", "enter").kind,
            ButtonKind::Primary
        );
    }

    #[test]
    fn kit_heights_are_32_36_44_56() {
        assert_eq!(HEIGHT_COMPACT, 32);
        assert_eq!(HEIGHT_ROW, 36);
        assert_eq!(HEIGHT_COMFORT, 44);
        assert_eq!(HEIGHT_TITLE, 48);
        assert_eq!(HEIGHT_RAIL, 48);
        assert_eq!(HEIGHT_CARD, 56);
        assert_eq!(TabSpec::height(), HEIGHT_ROW);
        assert_eq!(ButtonSpec::icon("⌘", "palette").height(), HEIGHT_COMPACT);
        assert_eq!(ButtonSpec::ghost("Hide", "h").height(), HEIGHT_COMPACT);
        assert_eq!(
            ButtonSpec::primary("Send", "enter").height(),
            HEIGHT_COMFORT
        );
        let danger = ButtonSpec {
            kind: ButtonKind::Danger,
            label: "Deny".into(),
            hint: "n".into(),
            icon: String::new(),
            enabled: true,
            busy: false,
        };
        assert_eq!(danger.height(), HEIGHT_COMFORT);
        let idle = ListRowSpec::new("a", "Alpha");
        assert_eq!(idle.height(), HEIGHT_CARD);
        assert_eq!(
            ListRowSpec {
                expanded: true,
                ..idle
            }
            .height(),
            HEIGHT_CARD
        );
    }

    #[test]
    fn empty_tiles_include_run_the_tests() {
        let tiles = empty_state_tiles();
        assert_eq!(
            tiles,
            [
                "What can you do?",
                "Summarize this repo",
                "git status",
                "Run the tests",
            ]
        );
        assert_eq!(EmptyStateSpec::tiles(), tiles);
        assert!(tiles.contains(&"Run the tests"));
    }
}
