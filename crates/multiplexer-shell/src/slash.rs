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
    Search,
    Settings,
    Files,
    Agents,
    Diff,
    Browser,
    Tui,
    About,
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
        "search" => SlashCommand::Search,
        "settings" => SlashCommand::Settings,
        "files" => SlashCommand::Files,
        "agents" => SlashCommand::Agents,
        "diff" | "diffs" => SlashCommand::Diff,
        "browser" => SlashCommand::Browser,
        "tui" => SlashCommand::Tui,
        "about" => SlashCommand::About,
        _ => SlashCommand::Unknown(token.to_string()),
    })
}

/// What Enter should do with the current draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendPlan {
    Slash(SlashCommand),
    StartTurn(String),
    IgnoreEmpty,
    IgnoreBusy,
}

/// Plan a send. `/stop` (and other slashes) run even while busy.
pub fn plan_send(draft: &str, busy: bool) -> SendPlan {
    let raw = draft.trim();
    if raw.is_empty() {
        return SendPlan::IgnoreEmpty;
    }
    if let Some(cmd) = parse_slash(raw) {
        return SendPlan::Slash(cmd);
    }
    if busy {
        return SendPlan::IgnoreBusy;
    }
    SendPlan::StartTurn(raw.to_owned())
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
        SlashCommand::Search => "open name search",
        SlashCommand::Settings => "open settings",
        SlashCommand::Files => "open project files",
        SlashCommand::Agents => "open local agents",
        SlashCommand::Diff => "open diffs",
        SlashCommand::Browser => "open browser tab",
        SlashCommand::Tui => "host the Grok TUI",
        SlashCommand::About => "open About",
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
            ("/search", SlashCommand::Search),
            ("/settings", SlashCommand::Settings),
            ("/files", SlashCommand::Files),
            ("/agents", SlashCommand::Agents),
            ("/diff", SlashCommand::Diff),
            ("/diffs", SlashCommand::Diff),
            ("/browser", SlashCommand::Browser),
            ("/tui", SlashCommand::Tui),
            ("/about", SlashCommand::About),
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
            SlashCommand::Search,
            SlashCommand::Settings,
            SlashCommand::Files,
            SlashCommand::Agents,
            SlashCommand::Diff,
            SlashCommand::Browser,
            SlashCommand::Tui,
            SlashCommand::About,
        ];
        for cmd in &cmds {
            let hint = slash_hint(cmd);
            assert!(!hint.is_empty(), "{cmd:?}");
            assert_ne!(hint, "unknown command", "{cmd:?}");
        }
        assert_eq!(slash_hint(&SlashCommand::New), "start a new chat");
        assert_eq!(slash_hint(&SlashCommand::Stop), "stop the running turn");
    }

    #[test]
    fn plan_send_stop_works_while_busy() {
        assert_eq!(plan_send("  ", false), SendPlan::IgnoreEmpty);
        assert_eq!(plan_send("", true), SendPlan::IgnoreEmpty);
        assert_eq!(
            plan_send("/stop", true),
            SendPlan::Slash(SlashCommand::Stop)
        );
        assert_eq!(
            plan_send("/help", true),
            SendPlan::Slash(SlashCommand::Help)
        );
        assert_eq!(plan_send("hello", true), SendPlan::IgnoreBusy);
        assert_eq!(
            plan_send("hello", false),
            SendPlan::StartTurn("hello".into())
        );
        assert_eq!(
            plan_send("/foo", false),
            SendPlan::Slash(SlashCommand::Unknown("foo".into()))
        );
    }
}
