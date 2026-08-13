//! Composer slash-command parse and hint text.

/// A leading `/token` typed in the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    New,
    Stop,
    Help,
    Checkpoint,
    Cores,
    Mcp,
    Points,
    Git,
    Terminal,
    Skills,
    Palette,
    Model,
    Unknown(String),
}

/// Parse a composer draft as a slash command.
///
/// Returns `None` unless `draft` (after leading whitespace) starts with `/`.
/// The first token after the slash is matched case-insensitively.
pub fn parse_slash(draft: &str) -> Option<SlashCommand> {
    let trimmed = draft.trim_start();
    let rest = trimmed.strip_prefix('/')?;
    let token = rest.split_whitespace().next().unwrap_or("");
    Some(match token.to_ascii_lowercase().as_str() {
        "new" => SlashCommand::New,
        "stop" => SlashCommand::Stop,
        "help" => SlashCommand::Help,
        "cp" | "checkpoint" => SlashCommand::Checkpoint,
        "cores" => SlashCommand::Cores,
        "mcp" => SlashCommand::Mcp,
        "points" => SlashCommand::Points,
        "git" => SlashCommand::Git,
        "term" | "terminal" => SlashCommand::Terminal,
        "skills" => SlashCommand::Skills,
        "palette" => SlashCommand::Palette,
        "model" => SlashCommand::Model,
        _ => SlashCommand::Unknown(token.to_string()),
    })
}

/// Short help copy for a parsed slash command.
pub fn slash_hint(cmd: &SlashCommand) -> &'static str {
    match cmd {
        SlashCommand::New => "start a new chat",
        SlashCommand::Stop => "stop the running turn",
        SlashCommand::Help => "show keyboard help",
        SlashCommand::Checkpoint => "create a checkpoint",
        SlashCommand::Cores => "open the cores inspector",
        SlashCommand::Mcp => "open the MCP inspector",
        SlashCommand::Points => "open the checkpoints inspector",
        SlashCommand::Git => "open the git inspector",
        SlashCommand::Terminal => "open the terminal",
        SlashCommand::Skills => "open skills",
        SlashCommand::Palette => "open the command palette",
        SlashCommand::Model => "switch model",
        SlashCommand::Unknown(_) => "unknown command",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_plain_text() {
        assert_eq!(parse_slash("hello"), None);
        assert_eq!(parse_slash("new"), None);
        assert_eq!(parse_slash("  stop"), None);
        assert_eq!(parse_slash(""), None);
        assert_eq!(parse_slash("   "), None);
        assert_eq!(parse_slash("hello /new"), None);
    }

    #[test]
    fn parses_aliases() {
        let cases: &[(&str, SlashCommand)] = &[
            ("/new", SlashCommand::New),
            ("/stop", SlashCommand::Stop),
            ("/help", SlashCommand::Help),
            ("/cp", SlashCommand::Checkpoint),
            ("/checkpoint", SlashCommand::Checkpoint),
            ("/cores", SlashCommand::Cores),
            ("/mcp", SlashCommand::Mcp),
            ("/points", SlashCommand::Points),
            ("/git", SlashCommand::Git),
            ("/term", SlashCommand::Terminal),
            ("/terminal", SlashCommand::Terminal),
            ("/skills", SlashCommand::Skills),
            ("/palette", SlashCommand::Palette),
            ("/model", SlashCommand::Model),
            ("/NEW extra", SlashCommand::New),
        ];
        for (draft, expected) in cases {
            assert_eq!(parse_slash(draft).as_ref(), Some(expected), "{draft}");
        }
    }

    #[test]
    fn unknown_keeps_name() {
        assert_eq!(
            parse_slash("/foo"),
            Some(SlashCommand::Unknown("foo".into()))
        );
        assert_eq!(
            parse_slash("/BarBaz"),
            Some(SlashCommand::Unknown("BarBaz".into()))
        );
        assert_eq!(parse_slash("/"), Some(SlashCommand::Unknown(String::new())));
        assert_eq!(
            slash_hint(&SlashCommand::Unknown("foo".into())),
            "unknown command"
        );
    }

    #[test]
    fn trims() {
        assert_eq!(parse_slash("  /new"), Some(SlashCommand::New));
        assert_eq!(parse_slash("\t/STOP"), Some(SlashCommand::Stop));
        assert_eq!(parse_slash("  /NEW extra  "), Some(SlashCommand::New));
        assert_eq!(
            parse_slash("  /checkpoint now"),
            Some(SlashCommand::Checkpoint)
        );
    }

    #[test]
    fn slash_hint_covers_every_known_command() {
        let cmds = [
            SlashCommand::New,
            SlashCommand::Stop,
            SlashCommand::Help,
            SlashCommand::Checkpoint,
            SlashCommand::Cores,
            SlashCommand::Mcp,
            SlashCommand::Points,
            SlashCommand::Git,
            SlashCommand::Terminal,
            SlashCommand::Skills,
            SlashCommand::Palette,
            SlashCommand::Model,
        ];
        for cmd in &cmds {
            let hint = slash_hint(cmd);
            assert!(!hint.is_empty(), "{cmd:?}");
            assert_ne!(hint, "unknown command", "{cmd:?}");
        }
        assert_eq!(slash_hint(&SlashCommand::New), "start a new chat");
        assert_eq!(slash_hint(&SlashCommand::Stop), "stop the running turn");
    }
}
