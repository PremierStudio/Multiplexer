//! Catalog helpers: models from grok config, RAM copy, remotes, open-external.

/// `[model.name]` / `[models.name]` table keys. Skip `op://` values and empty ids.
pub fn parse_model_keys(toml_text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut current: Option<String> = None;
    let mut skip = false;
    for raw in toml_text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if let Some(name) = current.take() {
                if !skip && !name.is_empty() && !names.iter().any(|n| n == &name) {
                    names.push(name);
                }
            }
            skip = false;
            current = table_model_name(line);
            continue;
        }
        if line.contains("op://") {
            skip = true;
        }
    }
    if let Some(name) = current {
        if !skip && !name.is_empty() && !names.iter().any(|n| n == &name) {
            names.push(name);
        }
    }
    names
}

fn table_model_name(header: &str) -> Option<String> {
    let inner = header.trim().trim_start_matches('[').trim_end_matches(']');
    let rest = inner
        .strip_prefix("model.")
        .or_else(|| inner.strip_prefix("models."))?;
    if rest.is_empty() || rest.contains('.') {
        return None;
    }
    Some(rest.to_owned())
}

/// Config ids first, then RPC ids. Drops empties and duplicates.
pub fn merge_models(config: &[String], rpc: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for name in config.iter().chain(rpc) {
        let name = name.trim();
        if name.is_empty() || name.contains("op://") {
            continue;
        }
        if !out.iter().any(|n| n == name) {
            out.push(name.to_owned());
        }
    }
    out
}

/// Token after the slash command, if any. `/model grok-4.6` -> `grok-4.6`.
pub fn slash_arg(draft: &str) -> Option<String> {
    let trimmed = draft.trim_start();
    let rest = trimmed.strip_prefix('/')?;
    let mut parts = rest.split_whitespace();
    let _cmd = parts.next()?;
    let arg = parts.next()?.trim();
    if arg.is_empty() {
        None
    } else {
        Some(arg.to_owned())
    }
}

/// Human RAM line. 0 is honest empty.
pub fn format_ram(bytes: u64) -> String {
    if bytes == 0 {
        return "RAM (not sampled)".to_owned();
    }
    if bytes >= 1024 * 1024 * 1024 {
        format!("RAM {:.1} GiB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else if bytes >= 1024 * 1024 {
        format!("RAM {:.0} MiB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("RAM {bytes} B")
    }
}

/// `DNSName` from `tailscale status --json`. None when missing.
pub fn parse_tailscale_dns(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let name = v.get("Self")?.get("DNSName")?.as_str()?;
    let name = name.trim().trim_end_matches('.');
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

/// `$VISUAL` else `$EDITOR` else system `start`.
pub fn open_external_program(visual: Option<&str>, editor: Option<&str>) -> String {
    if let Some(v) = visual.map(str::trim).filter(|s| !s.is_empty()) {
        return v.to_owned();
    }
    if let Some(e) = editor.map(str::trim).filter(|s| !s.is_empty()) {
        return e.to_owned();
    }
    "start".to_owned()
}

pub fn remotes_serve_note() -> &'static str {
    "Detect only. Tailscale Serve later."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_model_keys_skips_op_and_nested() {
        let toml = r#"
[model.grok]
name = "grok"
[model.ds-flash]
name = "flash"
api_key = "op://Vault/Item/field"
[models.extra]
id = "x"
[other.grok]
name = "nope"
[model.too.nested]
name = "no"
"#;
        let names = parse_model_keys(toml);
        assert!(names.contains(&"grok".into()));
        assert!(names.contains(&"extra".into()));
        assert!(!names.iter().any(|n| n == "ds-flash"));
        assert!(!names.iter().any(|n| n.contains("op://")));
        assert!(!names.iter().any(|n| n == "too"));
        assert!(parse_model_keys("not a table").is_empty());
        assert!(parse_model_keys("").is_empty());
    }

    #[test]
    fn merge_models_config_then_rpc() {
        let merged = merge_models(
            &["grok".into(), "".into(), "op://x".into()],
            &["grok".into(), "fake".into(), "  ".into()],
        );
        assert_eq!(merged, vec!["grok".to_owned(), "fake".to_owned()]);
        assert!(merge_models(&[], &[]).is_empty());
    }

    #[test]
    fn slash_arg_reads_second_token() {
        assert_eq!(slash_arg("/model grok-4.6").as_deref(), Some("grok-4.6"));
        assert_eq!(slash_arg("  /model   x  y").as_deref(), Some("x"));
        assert_eq!(slash_arg("/model"), None);
        assert_eq!(slash_arg("model grok"), None);
        assert_eq!(slash_arg("/files src"), Some("src".into()));
    }

    #[test]
    fn format_ram_bands() {
        assert_eq!(format_ram(0), "RAM (not sampled)");
        assert_eq!(format_ram(512), "RAM 512 B");
        assert!(format_ram(2 * 1024 * 1024).contains("MiB"));
        assert!(format_ram(3 * 1024 * 1024 * 1024).contains("GiB"));
        assert_ne!(format_ram(1), format_ram(0));
    }

    #[test]
    fn tailscale_dns_and_open_external() {
        assert_eq!(
            parse_tailscale_dns(r#"{"Self":{"DNSName":"box.tailnet.ts.net."}}"#).as_deref(),
            Some("box.tailnet.ts.net")
        );
        assert_eq!(parse_tailscale_dns("{}"), None);
        assert_eq!(parse_tailscale_dns("nope"), None);
        assert_eq!(parse_tailscale_dns(r#"{"Self":{"DNSName":""}}"#), None);
        assert_eq!(open_external_program(Some("code"), Some("vim")), "code");
        assert_eq!(open_external_program(Some("  "), Some("vim")), "vim");
        assert_eq!(open_external_program(None, None), "start");
        assert_eq!(remotes_serve_note(), "Detect only. Tailscale Serve later.");
    }
}
