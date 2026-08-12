//! Parse grok `config.toml` `[mcp_servers.<name>]` tables into inventory rows.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// One configured MCP server from a grok config document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpInventoryRow {
    pub name: String,
    pub command: String,
    pub transport: String,
}

/// Parse `[mcp_servers.<name>]` tables from grok config TOML text.
///
/// Invalid TOML and a missing `mcp_servers` table both yield an empty vec.
pub fn parse_mcp_inventory(toml_text: &str) -> Vec<McpInventoryRow> {
    let Ok(root) = toml_text.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(servers) = root.get("mcp_servers").and_then(toml::Value::as_table) else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    for (name, value) in servers {
        let Some(entry) = value.as_table() else {
            continue;
        };
        if let Some(command) = nonempty_str(entry, "command") {
            rows.push(McpInventoryRow {
                name: name.clone(),
                command: command.to_owned(),
                transport: "stdio".to_owned(),
            });
        } else if let Some(url) = nonempty_str(entry, "url") {
            rows.push(McpInventoryRow {
                name: name.clone(),
                command: url.to_owned(),
                transport: "http".to_owned(),
            });
        }
    }
    rows
}

fn nonempty_str<'a>(table: &'a toml::Table, key: &str) -> Option<&'a str> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|s| !s.is_empty())
}

/// Read inventory from `%USERPROFILE%/.grok/config.toml` or `~/.grok/config.toml`.
///
/// A missing file yields an empty vec.
pub fn load_user_mcp_inventory() -> Vec<McpInventoryRow> {
    load_user_mcp_inventory_at(std::env::var_os("USERPROFILE"), std::env::var_os("HOME"))
}

pub(crate) fn load_mcp_inventory_from(path: &Path) -> Vec<McpInventoryRow> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_mcp_inventory(&text),
        Err(_) => Vec::new(),
    }
}

pub(crate) fn load_user_mcp_inventory_at(
    userprofile: Option<OsString>,
    home: Option<OsString>,
) -> Vec<McpInventoryRow> {
    for path in user_config_candidates(userprofile, home) {
        if path.is_file() {
            return load_mcp_inventory_from(&path);
        }
    }
    Vec::new()
}

fn user_config_candidates(userprofile: Option<OsString>, home: Option<OsString>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for base in [userprofile, home].into_iter().flatten() {
        if base.is_empty() {
            continue;
        }
        let path = PathBuf::from(base).join(".grok").join("config.toml");
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_inventory_file_returns_empty() {
        let path = std::env::temp_dir()
            .join("multiplexer-mcp-missing")
            .join("definitely-not-there")
            .join("config.toml");
        assert!(!path.exists());
        assert!(load_mcp_inventory_from(&path).is_empty());
        assert!(load_user_mcp_inventory_at(None, None).is_empty());
        assert!(load_user_mcp_inventory_at(
            Some(
                std::env::temp_dir()
                    .join("multiplexer-mcp-missing-home")
                    .into()
            ),
            None
        )
        .is_empty());
    }

    #[test]
    fn user_config_paths_join_grok_config_toml() {
        let paths = user_config_candidates(
            Some(OsString::from("C:/Users/example")),
            Some(OsString::from("/home/example")),
        );
        assert_eq!(
            paths,
            vec![
                PathBuf::from("C:/Users/example")
                    .join(".grok")
                    .join("config.toml"),
                PathBuf::from("/home/example")
                    .join(".grok")
                    .join("config.toml"),
            ]
        );
    }
}
