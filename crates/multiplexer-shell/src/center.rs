//! Center pane: thin GUI log vs host for the real Grok TUI.

/// Which surface occupies the Outlook center.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CenterMode {
    /// Headless transcript (`grok -p`). Not the Grok pager.
    #[default]
    Gui,
    /// Host for the interactive Grok TUI (new console, not a GPUI rewrite).
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

/// Supervised Grok pager process (new console). Not an in-pane VT emulator.
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
}

impl GrokTuiHost {
    pub fn idle(cwd: impl Into<String>) -> Self {
        Self {
            life: TuiLife::Stopped,
            pid: None,
            program: "grok".into(),
            cwd: cwd.into(),
            note: "Grok owns the agent TUI. Multiplexer hosts it in a real console. In-pane ConPTY is later.".into(),
            surface: "new console".into(),
        }
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
    fn host_lifecycle_is_supervised_not_embedded() {
        let mut host = GrokTuiHost::idle("C:/repo");
        assert_eq!(host.life, TuiLife::Stopped);
        assert!(host.pid.is_none());
        assert!(host.note.contains("real console"));
        assert!(host.summary().contains("stopped"));

        host.mark_running(Some(4242), "grok", "new console");
        assert_eq!(host.life, TuiLife::Running);
        assert_eq!(host.pid, Some(4242));
        assert!(host.summary().contains("4242"));
        assert!(host.summary().contains("running"));
        assert!(host.summary().contains("new console"));
        assert!(!host.summary().to_ascii_lowercase().contains("embedded"));
        assert!(!host.summary().contains("in-pane pager"));

        host.mark_exited();
        assert_eq!(host.life, TuiLife::Exited);
        assert!(host.pid.is_none());

        host.mark_failed("grok not on PATH");
        assert_eq!(host.life, TuiLife::Failed);
        assert!(host.note.contains("PATH"));
        assert_ne!(
            host.note,
            "Grok owns the agent TUI. Multiplexer hosts it in a real console. In-pane ConPTY is later."
        );
    }

    #[test]
    fn summary_names_surface_not_embedded() {
        let mut host = GrokTuiHost::idle("C:/repo");
        host.mark_running(None, "grok", "Windows Terminal");
        let s = host.summary();
        assert!(s.contains("Windows Terminal"));
        assert!(!s.contains("embedded"));
        assert_eq!(host.life, TuiLife::Running);
        assert!(host.pid.is_none());
    }
}
