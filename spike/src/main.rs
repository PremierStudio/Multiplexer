//! Phase-0 spike: prove `xai-grok-shell` is consumable as a Rust library
//! from an independent binary on Windows, via path-dependency wiring
//! (the D5 vendored-fork + `[patch]` model).
//!
//! This exercises:
//! 1. The shell crate compiles as a *dependency* of an external crate (not
//!    just as a workspace member in isolation).
//! 2. Public, non-auth APIs are reachable: `xai-grok-version::VERSION` and
//!    the shell's config loader (`xai_grok_shell::config::load_effective_config`).
//! 3. The config model loads our user config (the same `[model.*]` /
//!    `[auth_provider.*]` dialect the plan depends on for `ds-flash`).

fn main() {
    println!("xai-grok-version::VERSION      = {}", xai_grok_version::VERSION);
    println!("xai-grok-version::installed()  = {}", xai_grok_version::installed());
    println!("xai_grok_version (semver)      = {:?}", xai_grok_version::installed_semver());

    // Prove the shell crate's public module surface resolves: config types,
    // the agent Config, and session types are all reachable.
    let _agent_cfg: Option<xai_grok_shell::agent::config::Config> = None;
    let _prompt_origin: xai_grok_shell::session::PromptOrigin =
        xai_grok_shell::session::PromptOrigin::User;
    println!("shell public API surface resolves: config, agent::config, session::PromptOrigin");

    // Load the effective user config the way the CLI does. If the user runs
    // grok (they do), ~/.grok/config.toml exists and parses.
    match xai_grok_shell::config::load_effective_config() {
        Ok(toml_value) => {
            println!("load_effective_config() -> OK (toml::Value)");
            let models = toml_value
                .get("model")
                .and_then(|m| m.as_table())
                .map(|t| t.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let models_str = if models.is_empty() { "<none>".to_string() } else { models.join(", ") };
            println!("  [model.*] entries: {models_str}");
            let auth_providers = toml_value
                .get("auth_provider")
                .and_then(|m| m.as_table())
                .map(|t| t.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            let auth_str = if auth_providers.is_empty() { "<none>".to_string() } else { auth_providers.join(", ") };
            println!("  [auth_provider.*] entries: {auth_str}");
        }
        Err(e) => println!("load_effective_config() -> (expected if not logged in) {e}"),
    }

    // Show the user's grok home so we know which config was read.
    let home = xai_grok_config::grok_home();
    println!("grok home                      = {}", home.display());

    println!();
    println!("SPIKE-OK: xai-grok-shell is consumable as a library on Windows");
}