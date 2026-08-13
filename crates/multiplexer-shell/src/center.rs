//! Center pane: thin GUI log vs host for the real Grok TUI.

/// Which surface occupies the Outlook center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CenterMode {
    /// Headless transcript (`grok -p`). Not the Grok pager.
    #[default]
    Gui,
    /// Host for the interactive Grok TUI (in-app session, not a GPUI pager rewrite).
    GrokTui,
}

impl CenterMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Gui => "Chat log",
            Self::GrokTui => "Grok TUI",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::Gui => Self::GrokTui,
            Self::GrokTui => Self::Gui,
        }
    }
}

/// Supervised Grok pager process. Output is hosted in-app when ConPTY is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TuiLife {
    #[default]
    Stopped,
    Running,
    Exited,
    Failed,
}

impl TuiLife {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Exited => "exited",
            Self::Failed => "failed",
        }
    }
}

/// Projection of the Grok TUI host. The OS process lives in the desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrokTuiHost {
    pub life: TuiLife,
    pub pid: Option<u32>,
    pub program: String,
    pub cwd: String,
    pub note: String,
    pub surface: String,
    pub scrollback: String,
}

impl GrokTuiHost {
    pub fn idle(cwd: impl Into<String>) -> Self {
        Self {
            life: TuiLife::Stopped,
            pid: None,
            program: "grok".into(),
            cwd: cwd.into(),
            note: "Grok owns the agent TUI. Multiplexer hosts the system shell in-app.".into(),
            surface: "in-app".into(),
            scrollback: String::new(),
        }
    }

    pub fn push_output(&mut self, chunk: &str) {
        self.scrollback.push_str(chunk);
        const CAP: usize = 64 * 1024;
        self.scrollback = keep_tail(&self.scrollback, CAP);
    }

    /// Replace the painted frame. Used for last-frame PTY text so a
    /// grok pager redraw does not append leftover ASCII.
    pub fn set_output(&mut self, text: &str) {
        const CAP: usize = 64 * 1024;
        self.scrollback = keep_tail(text, CAP);
    }

    pub fn mark_running(
        &mut self,
        pid: Option<u32>,
        program: impl Into<String>,
        surface: impl Into<String>,
    ) {
        self.life = TuiLife::Running;
        self.pid = pid;
        self.program = program.into();
        self.surface = surface.into();
    }

    pub fn mark_exited(&mut self) {
        self.life = TuiLife::Exited;
        self.pid = None;
    }

    pub fn mark_failed(&mut self, message: impl Into<String>) {
        self.life = TuiLife::Failed;
        self.pid = None;
        self.note = message.into();
    }

    pub fn summary(&self) -> String {
        format!(
            "{}  {}  {}  pid {}  {}\n{}",
            self.life.label(),
            self.surface,
            self.program,
            self.pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".into()),
            self.cwd,
            self.note
        )
    }
}

/// Keep the last `max` bytes of `text`, never splitting a UTF-8 character.
pub fn keep_tail(text: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if text.len() <= max {
        return text.to_owned();
    }
    let cut = text.floor_char_boundary(text.len() - max);
    text[cut..].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_swaps_gui_and_tui() {
        assert_eq!(CenterMode::Gui.label(), "Chat log");
        assert_eq!(CenterMode::GrokTui.label(), "Grok TUI");
        assert_eq!(CenterMode::Gui.toggle(), CenterMode::GrokTui);
        assert_eq!(CenterMode::GrokTui.toggle(), CenterMode::Gui);
        assert_ne!(CenterMode::Gui, CenterMode::GrokTui);
        assert_eq!(CenterMode::default(), CenterMode::Gui);
    }

    #[test]
    fn host_lifecycle_is_supervised() {
        let mut host = GrokTuiHost::idle("C:/repo");
        assert_eq!(host.life, TuiLife::Stopped);
        assert!(host.pid.is_none());
        assert!(host.note.contains("in-app"));
        assert!(host.summary().contains("stopped"));
        assert!(host.scrollback.is_empty());

        host.mark_running(Some(4242), "grok", "Windows Terminal");
        assert_eq!(host.life, TuiLife::Running);
        assert_eq!(host.pid, Some(4242));
        assert!(host.summary().contains("4242"));
        assert!(host.summary().contains("running"));
        assert!(host.summary().contains("Windows Terminal"));
        assert!(!host.summary().contains("in-pane pager"));
        host.push_output("hello from grok\n");
        assert!(host.scrollback.contains("hello from grok"));
        host.set_output("frame one");
        assert_eq!(host.scrollback, "frame one");
        assert!(!host.scrollback.contains("hello from grok"));
        host.set_output("frame two");
        assert_eq!(host.scrollback, "frame two");
        assert_ne!(host.scrollback, "frame oneframe two");
        let huge = format!("{}TAIL", "⠀".repeat(22_000));
        host.set_output(&huge);
        assert!(host.scrollback.ends_with("TAIL"));
        assert!(host.scrollback.len() > 2000);
        assert!(host.scrollback.len() <= 64 * 1024);
        assert!(host.scrollback.is_char_boundary(0));

        host.mark_exited();
        assert_eq!(host.life, TuiLife::Exited);
        assert!(host.pid.is_none());

        host.mark_failed("grok not on PATH");
        assert_eq!(host.life, TuiLife::Failed);
        assert!(host.note.contains("PATH"));
    }

    #[test]
    fn summary_names_surface() {
        let mut host = GrokTuiHost::idle("C:/repo");
        host.mark_running(None, "grok", "Windows Terminal");
        let s = host.summary();
        assert!(s.contains("Windows Terminal"));
        assert_eq!(host.life, TuiLife::Running);
        assert!(host.pid.is_none());
    }

    #[test]
    fn keep_tail_does_not_split_multibyte_chars() {
        assert_eq!(keep_tail("hello", 10), "hello");
        assert_eq!(keep_tail("abcdef", 3), "def");
        assert_eq!(keep_tail("", 8), "");
        assert_eq!(keep_tail("abc", 0), "");
        let braille = "⠀";
        assert_eq!(braille.len(), 3);
        let text = format!("{}{}", braille.repeat(20), "z".repeat(8));
        let tail = keep_tail(&text, 10);
        assert!(tail.is_char_boundary(0));
        assert!(tail.ends_with("zzzzzzzz"));
        assert!(tail.starts_with(braille));
        assert!(!text.is_char_boundary(text.len() - 10));
        assert_eq!(keep_tail(braille, 1), braille);
        assert_ne!(keep_tail(braille, 1), "");
        let mut host = GrokTuiHost::idle(".");
        host.scrollback = "⠀".repeat(22_000);
        host.push_output("more");
        assert!(host.scrollback.is_char_boundary(0));
        assert!(host.scrollback.len() <= 64 * 1024);
    }
}
