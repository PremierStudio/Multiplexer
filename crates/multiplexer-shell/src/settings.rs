//! User settings the Settings overlay can mutate.

use multiplexer_theme::{Density, ThemeMode};

/// Persisted-in-memory UI settings. Disk later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiSettings {
    pub mode: ThemeMode,
    pub density: Density,
    pub default_model: String,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            mode: ThemeMode::Dark,
            density: Density::Comfortable,
            default_model: "grok".into(),
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
    }
}
