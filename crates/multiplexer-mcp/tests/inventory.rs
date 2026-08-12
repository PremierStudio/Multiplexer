//! Parse grok `[mcp_servers.<name>]` inventory from TOML text.

use multiplexer_mcp::{parse_mcp_inventory, McpInventoryRow};
use proptest::prelude::*;

fn names(rows: &[McpInventoryRow]) -> Vec<&str> {
    rows.iter().map(|r| r.name.as_str()).collect()
}

#[test]
fn stdio_command_table() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.foo]
command = "npx"
"#,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "foo");
    assert_eq!(rows[0].command, "npx");
    assert_eq!(rows[0].transport, "stdio");
    assert_ne!(rows[0].transport, "http");
    assert_ne!(rows[0].transport, "");
}

#[test]
fn http_url_table() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.bar]
url = "https://mcp.example.com/mcp"
"#,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "bar");
    assert_eq!(rows[0].command, "https://mcp.example.com/mcp");
    assert_eq!(rows[0].transport, "http");
    assert_ne!(rows[0].transport, "stdio");
    assert_ne!(rows[0].transport, "");
}

#[test]
fn mixed_stdio_and_http() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.foo]
command = "npx"
args = ["-y", "@pkg"]

[mcp_servers.bar]
url = "https://mcp.example.com/mcp"
"#,
    );
    assert_eq!(rows.len(), 2);
    let foo = rows.iter().find(|r| r.name == "foo").expect("foo");
    assert_eq!(foo.command, "npx");
    assert_eq!(foo.transport, "stdio");
    let bar = rows.iter().find(|r| r.name == "bar").expect("bar");
    assert_eq!(bar.command, "https://mcp.example.com/mcp");
    assert_eq!(bar.transport, "http");
}

#[test]
fn inline_table_form() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers]
foo = { command = "npx" }
bar = { url = "https://mcp.example.com/mcp" }
"#,
    );
    assert_eq!(rows.len(), 2);
    let foo = rows.iter().find(|r| r.name == "foo").expect("foo");
    assert_eq!(foo.command, "npx");
    assert_eq!(foo.transport, "stdio");
    let bar = rows.iter().find(|r| r.name == "bar").expect("bar");
    assert_eq!(bar.command, "https://mcp.example.com/mcp");
    assert_eq!(bar.transport, "http");
}

#[test]
fn missing_table_is_empty() {
    assert!(parse_mcp_inventory("").is_empty());
    assert!(parse_mcp_inventory("# just a comment\n").is_empty());
    assert!(parse_mcp_inventory("[models.ds_flash]\nname = \"x\"\n").is_empty());
    assert!(parse_mcp_inventory("mcp_servers = \"nope\"\n").is_empty());
}

#[test]
fn invalid_toml_is_empty() {
    assert!(parse_mcp_inventory("this is not = [ toml").is_empty());
    assert!(parse_mcp_inventory("[unclosed").is_empty());
    assert!(parse_mcp_inventory("[[[").is_empty());
}

#[test]
fn empty_mcp_servers_table_is_empty() {
    assert!(parse_mcp_inventory("[mcp_servers]\n").is_empty());
}

#[test]
fn skips_entries_without_command_or_url() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.ok]
command = "npx"

[mcp_servers.nope]
enabled = true

[mcp_servers]
stray = "value"
"#,
    );
    assert_eq!(names(&rows), vec!["ok"]);
}

#[test]
fn empty_command_does_not_become_stdio() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.blank]
command = ""

[mcp_servers.fallback]
command = ""
url = "https://x.example/mcp"
"#,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "fallback");
    assert_eq!(rows[0].command, "https://x.example/mcp");
    assert_eq!(rows[0].transport, "http");
}

#[test]
fn empty_url_is_skipped() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.blank]
url = ""
"#,
    );
    assert!(rows.is_empty());
}

#[test]
fn command_wins_over_url() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.both]
command = "npx"
url = "https://ignored.example/mcp"
"#,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].command, "npx");
    assert_eq!(rows[0].transport, "stdio");
}

#[test]
fn hyphenated_server_name() {
    let rows = parse_mcp_inventory(
        r#"
[mcp_servers.my-server]
command = "uvx"
"#,
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "my-server");
    assert_eq!(rows[0].command, "uvx");
    assert_eq!(rows[0].transport, "stdio");
}

proptest! {
    #[test]
    fn parse_mcp_inventory_never_panics(s in "\\PC*") {
        let rows = parse_mcp_inventory(&s);
        for row in &rows {
            prop_assert!(!row.name.is_empty());
            prop_assert!(!row.command.is_empty());
            prop_assert!(row.transport == "stdio" || row.transport == "http");
        }
    }
}
