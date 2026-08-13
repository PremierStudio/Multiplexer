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
pub fn embed_from_selection(term: Option<&SystemTerminal>, cwd: &str) -> EmbedTarget {
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
        _ => embed_grok(cwd),
    }
}

/// Interactive Grok for a chat. `--trust` skips the folder gate;
/// `--always-approve` skips tool prompts; no `-p` so the pager stays up.
pub fn embed_grok(cwd: impl AsRef<str>) -> EmbedTarget {
    let cwd = cwd.as_ref().trim();
    let cwd = if cwd.is_empty() { "." } else { cwd };
    EmbedTarget {
        program: "grok".into(),
        args: vec![
            "--always-approve".into(),
            "--trust".into(),
            "--cwd".into(),
            cwd.to_owned(),
        ],
        label: "Grok".into(),
    }
}

/// Surface string painted in the in-app host chrome.
pub fn embed_surface(label: &str) -> String {
    format!("in-app {label}")
}

/// GUI Send types into the live grok PTY instead of starting `grok -p`.
pub fn live_pty_takes_gui_send(tui_alive: bool) -> bool {
    tui_alive
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
        let got = embed_from_selection(Some(&t), "C:/repo");
        assert_eq!(got.program, t.path);
        assert!(got.args.is_empty());
        assert_eq!(got.label, "Command Prompt");
        assert_ne!(got.program, "grok");
        assert_eq!(embed_surface(&got.label), "in-app Command Prompt");
    }

    #[test]
    fn empty_or_missing_falls_back_to_grok() {
        assert_eq!(embed_from_selection(None, "C:/repo"), embed_grok("C:/repo"));
        let blank = term("x", "X", "   ");
        assert_eq!(
            embed_from_selection(Some(&blank), "C:/repo").program,
            "grok"
        );
        let no_label = term("zsh", "  ", "/bin/zsh");
        assert_eq!(embed_from_selection(Some(&no_label), ".").label, "zsh");
        assert_eq!(embed_grok("C:/repo").program, "grok");
        assert_eq!(embed_grok("C:/repo").label, "Grok");
        assert_ne!(embed_grok("C:/repo").label, "grok");
        assert_eq!(embed_surface("Grok"), "in-app Grok");
        assert_ne!(embed_surface("Grok"), "Grok");
    }

    #[test]
    fn grok_skips_folder_gate_and_tool_prompts() {
        let got = embed_grok("C:/work/app");
        assert_eq!(
            got.args,
            vec!["--always-approve", "--trust", "--cwd", "C:/work/app"]
        );
        assert!(!got.args.iter().any(|a| a == "-p"));
        assert_ne!(got.args, embed_grok("D:/other").args);
        assert_eq!(embed_grok("   ").args.last().map(String::as_str), Some("."));
        assert_eq!(embed_grok("").args[2], "--cwd");
        assert_ne!(embed_grok("C:/work/app").args.len(), 0);
    }

    #[test]
    fn gui_send_uses_live_pty_only_when_alive() {
        assert!(live_pty_takes_gui_send(true));
        assert!(!live_pty_takes_gui_send(false));
        assert_ne!(
            live_pty_takes_gui_send(true),
            live_pty_takes_gui_send(false)
        );
    }
}
