//! Decide which system shell or grok to host in the in-app PTY.

use crate::workbench::SystemTerminal;

/// Program, argv, and label for [`multiplexer_terminal::EmbeddedSession::spawn`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedTarget {
    pub program: String,
    pub args: Vec<String>,
    pub label: String,
}

/// Open-here: selected shell path, or grok if nothing was detected.
pub fn embed_from_selection(term: Option<&SystemTerminal>) -> EmbedTarget {
    match term {
        Some(t) if !t.path.trim().is_empty() => EmbedTarget {
            program: t.path.clone(),
            args: Vec::new(),
            label: if t.label.trim().is_empty() {
                t.id.clone()
            } else {
                t.label.clone()
            },
        },
        _ => embed_grok(),
    }
}

/// Open Grok in the pane, independent of the selected shell.
pub fn embed_grok() -> EmbedTarget {
    EmbedTarget {
        program: "grok".into(),
        args: Vec::new(),
        label: "Grok".into(),
    }
}

/// Surface string painted in the in-app host chrome.
pub fn embed_surface(label: &str) -> String {
    format!("in-app {label}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(id: &str, label: &str, path: &str) -> SystemTerminal {
        SystemTerminal {
            id: id.into(),
            label: label.into(),
            path: path.into(),
        }
    }

    #[test]
    fn selection_uses_path_and_label() {
        let t = term("cmd", "Command Prompt", r"C:\Windows\System32\cmd.exe");
        let got = embed_from_selection(Some(&t));
        assert_eq!(got.program, t.path);
        assert!(got.args.is_empty());
        assert_eq!(got.label, "Command Prompt");
        assert_ne!(got.program, "grok");
        assert_eq!(embed_surface(&got.label), "in-app Command Prompt");
    }

    #[test]
    fn empty_or_missing_falls_back_to_grok() {
        assert_eq!(embed_from_selection(None), embed_grok());
        let blank = term("x", "X", "   ");
        assert_eq!(embed_from_selection(Some(&blank)).program, "grok");
        let no_label = term("zsh", "  ", "/bin/zsh");
        assert_eq!(embed_from_selection(Some(&no_label)).label, "zsh");
        assert_eq!(embed_grok().program, "grok");
        assert_eq!(embed_grok().label, "Grok");
        assert!(embed_grok().args.is_empty());
        assert_ne!(embed_grok().label, "grok");
        assert_eq!(embed_surface("Grok"), "in-app Grok");
        assert_ne!(embed_surface("Grok"), "Grok");
    }
}
