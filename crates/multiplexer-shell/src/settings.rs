//! User settings the Settings page can mutate and persist.

use std::path::{Path, PathBuf};

use multiplexer_theme::{Density, ThemeMode};

use crate::keymap::BindingTable;

/// Settings nav on the exclusive Settings page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Appearance,
    Models,
    Bindings,
    Inspector,
    Session,
    Remotes,
    About,
}

impl SettingsSection {
    pub fn all() -> [Self; 7] {
        [
            Self::Appearance,
            Self::Models,
            Self::Bindings,
            Self::Inspector,
            Self::Session,
            Self::Remotes,
            Self::About,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Models => "Models",
            Self::Bindings => "Bindings",
            Self::Inspector => "Inspector",
            Self::Session => "Session",
            Self::Remotes => "Remotes",
            Self::About => "About",
        }
    }
}

/// Persisted UI settings. No secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSettings {
    pub mode: ThemeMode,
    pub density: Density,
    pub default_model: String,
    pub bindings: Vec<(String, String)>,
    pub reduce_motion: bool,
    pub ui_scale: u16,
    pub high_contrast: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            density: Density::Comfortable,
            default_model: "grok".into(),
            bindings: BindingTable::defaults().pairs(),
            reduce_motion: false,
            ui_scale: 100,
            high_contrast: false,
        }
    }
}

impl UiSettings {
    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
        };
    }

    pub fn cycle_density(&mut self) {
        self.density = match self.density {
            Density::Comfortable => Density::Compact,
            Density::Compact => Density::Comfortable,
        };
    }

    pub fn set_default_model(&mut self, model: impl Into<String>) {
        let model = model.into();
        if !model.trim().is_empty() {
            self.default_model = model;
        }
    }

    pub fn binding_table(&self) -> BindingTable {
        if self.bindings.is_empty() {
            BindingTable::defaults()
        } else {
            BindingTable::from_pairs(&self.bindings)
        }
    }

    pub fn clamp_ui_scale(scale: u16) -> u16 {
        scale.clamp(100, 200)
    }

    pub fn set_ui_scale(&mut self, scale: u16) {
        self.ui_scale = Self::clamp_ui_scale(scale);
    }

    pub fn bump_ui_scale(&mut self) {
        let next = if self.ui_scale >= 200 {
            100
        } else {
            self.ui_scale.saturating_add(25)
        };
        self.set_ui_scale(next);
    }

    pub fn toggle_reduce_motion(&mut self) {
        self.reduce_motion = !self.reduce_motion;
    }

    pub fn toggle_high_contrast(&mut self) {
        self.high_contrast = !self.high_contrast;
    }
}

/// `%APPDATA%\Multiplexer\settings.json` (HOME fallback).
pub fn default_settings_path() -> PathBuf {
    let root = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("Multiplexer").join("settings.json")
}

pub fn settings_to_json(s: &UiSettings) -> String {
    let mode = match s.mode {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    };
    let density = match s.density {
        Density::Comfortable => "comfortable",
        Density::Compact => "compact",
    };
    let bindings: Vec<serde_json::Value> = s
        .bindings
        .iter()
        .map(|(c, a)| serde_json::json!([c, a]))
        .collect();
    serde_json::json!({
        "mode": mode,
        "density": density,
        "default_model": s.default_model,
        "bindings": bindings,
        "reduce_motion": s.reduce_motion,
        "ui_scale": s.ui_scale,
        "high_contrast": s.high_contrast,
    })
    .to_string()
}

pub fn settings_from_json(raw: &str) -> UiSettings {
    let mut out = UiSettings::default();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return out;
    };
    if let Some(mode) = v.get("mode").and_then(|x| x.as_str()) {
        out.mode = match mode {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Dark,
        };
    }
    if let Some(density) = v.get("density").and_then(|x| x.as_str()) {
        out.density = match density {
            "compact" => Density::Compact,
            "comfortable" => Density::Comfortable,
            _ => Density::Comfortable,
        };
    }
    if let Some(model) = v.get("default_model").and_then(|x| x.as_str()) {
        if !model.trim().is_empty() {
            out.default_model = model.to_owned();
        }
    }
    if let Some(arr) = v.get("bindings").and_then(|x| x.as_array()) {
        let mut pairs = Vec::new();
        for item in arr {
            if let Some(pair) = item.as_array() {
                if pair.len() == 2 {
                    if let (Some(c), Some(a)) = (pair[0].as_str(), pair[1].as_str()) {
                        pairs.push((c.to_owned(), a.to_owned()));
                    }
                }
            }
        }
        if !pairs.is_empty() {
            out.bindings = BindingTable::from_pairs(&pairs).pairs();
        }
    }
    if let Some(flag) = v.get("reduce_motion").and_then(|x| x.as_bool()) {
        out.reduce_motion = flag;
    }
    if let Some(scale) = v.get("ui_scale").and_then(|x| x.as_u64()) {
        out.set_ui_scale(scale as u16);
    }
    if let Some(flag) = v.get("high_contrast").and_then(|x| x.as_bool()) {
        out.high_contrast = flag;
    }
    out
}

pub fn write_settings(path: &Path, s: &UiSettings) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, settings_to_json(s)).map_err(|e| e.to_string())
}

pub fn read_settings(path: &Path) -> UiSettings {
    match std::fs::read_to_string(path) {
        Ok(raw) => settings_from_json(&raw),
        Err(_) => UiSettings::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_cycle_mode_and_density() {
        let mut s = UiSettings::default();
        assert_eq!(s.mode, ThemeMode::Dark);
        s.cycle_mode();
        assert_eq!(s.mode, ThemeMode::Light);
        s.cycle_mode();
        assert_eq!(s.mode, ThemeMode::Dark);
        s.cycle_density();
        assert_eq!(s.density, Density::Compact);
        s.set_default_model("grok-4.6");
        assert_eq!(s.default_model, "grok-4.6");
        s.set_default_model("  ");
        assert_eq!(s.default_model, "grok-4.6");
        assert!(!s.bindings.is_empty());
        assert_eq!(
            s.binding_table().lookup_spec("ctrl-p"),
            Some(crate::ClientAction::ToggleSearch)
        );
    }

    #[test]
    fn json_roundtrip_and_invalid() {
        let mut s = UiSettings::default();
        s.cycle_mode();
        s.cycle_density();
        s.set_default_model("grok-4.6");
        let raw = settings_to_json(&s);
        assert!(raw.contains("\"light\""));
        assert!(raw.contains("\"compact\""));
        assert!(raw.contains("grok-4.6"));
        assert!(!raw.to_lowercase().contains("secret"));
        assert!(!raw.contains("op://"));
        let back = settings_from_json(&raw);
        assert_eq!(back.mode, ThemeMode::Light);
        assert_eq!(back.density, Density::Compact);
        assert_eq!(back.default_model, "grok-4.6");
        assert_eq!(
            back.binding_table().lookup_spec("ctrl-shift-p"),
            Some(crate::ClientAction::TogglePalette)
        );

        let broken = settings_from_json("not-json");
        assert_eq!(broken, UiSettings::default());
        let empty_model =
            settings_from_json(r#"{"mode":"nope","density":"nope","default_model":"  "}"#);
        assert_eq!(empty_model.mode, ThemeMode::Dark);
        assert_eq!(empty_model.density, Density::Comfortable);
        assert_eq!(empty_model.default_model, "grok");
        let light_only = settings_from_json(r#"{"mode":"light"}"#);
        assert_eq!(light_only.mode, ThemeMode::Light);
        assert_eq!(light_only.density, Density::Comfortable);
        let rebound = settings_from_json(
            r#"{"bindings":[["ctrl-p","command_palette"],["not-a-pair"],["ctrl-k","help"]]}"#,
        );
        assert_eq!(
            rebound.binding_table().lookup_spec("ctrl-p"),
            Some(crate::ClientAction::TogglePalette)
        );
        assert_eq!(
            rebound.binding_table().lookup_spec("ctrl-k"),
            Some(crate::ClientAction::ToggleHelp)
        );
        let custom = UiSettings {
            bindings: vec![("ctrl-p".into(), "command_palette".into())],
            ..UiSettings::default()
        };
        assert_eq!(
            custom.binding_table().lookup_spec("ctrl-p"),
            Some(crate::ClientAction::TogglePalette)
        );
        assert_ne!(
            custom.binding_table().lookup_spec("ctrl-p"),
            UiSettings::default().binding_table().lookup_spec("ctrl-p")
        );
    }

    #[test]
    fn write_and_read_temp() {
        let dir = std::env::temp_dir().join(format!(
            "mux-settings-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(1)
        ));
        let path = dir.join("settings.json");
        let mut s = UiSettings::default();
        s.cycle_mode();
        write_settings(&path, &s).expect("write");
        let loaded = read_settings(&path);
        assert_eq!(loaded.mode, ThemeMode::Light);
        assert_eq!(
            read_settings(Path::new("C:/no/such/mux-settings.json")),
            UiSettings::default()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_section_labels() {
        assert_eq!(SettingsSection::all().len(), 7);
        assert_eq!(SettingsSection::Appearance.label(), "Appearance");
        assert_eq!(SettingsSection::Remotes.label(), "Remotes");
        assert_eq!(SettingsSection::About.label(), "About");
        assert_ne!(
            SettingsSection::Models.label(),
            SettingsSection::Bindings.label()
        );
        assert_ne!(SettingsSection::Inspector.label(), "");
    }

    #[test]
    fn a11y_fields_clamp_and_toggle() {
        let mut s = UiSettings::default();
        assert!(!s.reduce_motion);
        assert!(!s.high_contrast);
        assert_eq!(s.ui_scale, 100);
        s.toggle_reduce_motion();
        s.toggle_high_contrast();
        s.set_ui_scale(50);
        assert_eq!(s.ui_scale, 100);
        s.set_ui_scale(250);
        assert_eq!(s.ui_scale, 200);
        s.set_ui_scale(125);
        assert_eq!(s.ui_scale, 125);
        s.ui_scale = 200;
        s.bump_ui_scale();
        assert_eq!(s.ui_scale, 100);
        s.bump_ui_scale();
        assert_eq!(s.ui_scale, 125);
        let raw = settings_to_json(&s);
        assert!(raw.contains("reduce_motion"));
        assert!(raw.contains("ui_scale"));
        let back = settings_from_json(&raw);
        assert!(back.reduce_motion);
        assert!(back.high_contrast);
        let scaled = settings_from_json(r#"{"ui_scale":180,"reduce_motion":true}"#);
        assert_eq!(scaled.ui_scale, 180);
        assert!(scaled.reduce_motion);
    }

    #[test]
    fn default_path_ends_with_settings_json() {
        let p = default_settings_path();
        assert_eq!(
            p.file_name().and_then(|s| s.to_str()),
            Some("settings.json")
        );
        assert!(
            p.parent()
                .and_then(|s| s.file_name())
                .and_then(|s| s.to_str())
                == Some("Multiplexer")
        );
    }

    #[test]
    fn empty_bindings_uses_defaults() {
        let mut s = UiSettings::default();
        s.bindings.clear();
        assert_eq!(
            s.binding_table().lookup_spec("ctrl-k"),
            Some(crate::ClientAction::TogglePalette)
        );
    }
}
