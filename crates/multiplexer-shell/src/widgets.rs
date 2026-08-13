//! Headless UI specs. Desktop projects these into GPUI.

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
            ButtonKind::Icon => 32,
            _ => 36,
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
        if self.expanded {
            88
        } else {
            44
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmptyStateSpec {
    pub title: String,
    pub body: String,
    pub action: String,
}

impl EmptyStateSpec {
    pub fn chat() -> Self {
        Self {
            title: "Start a session".into(),
            body: "Run a real grok turn, open a worktree, or inspect MCP and cores.".into(),
            action: "New chat".into(),
        }
    }
}

/// Suggestion tiles on the empty center.
pub fn empty_state_tiles() -> [&'static str; 4] {
    [
        "What can you do?",
        "Summarize this repo",
        "git status",
        "List project files",
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
        assert_eq!(idle.height(), 44);
        assert_eq!(
            ListRowSpec {
                expanded: true,
                ..idle.clone()
            }
            .height(),
            88
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
        assert_eq!(ButtonSpec::icon("⌘", "palette").height(), 32);
        assert_eq!(
            ButtonSpec::primary("Send", "enter").kind,
            ButtonKind::Primary
        );
    }
}
