//! Modal overlay policy for the Outlook chrome.

/// One exclusive or stackable overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Palette,
    Help,
    Settings,
    Search,
}

/// Painted overlay flags. Settings and Search replace. Palette and Help stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlayFlags {
    pub palette: bool,
    pub help: bool,
    pub settings: bool,
    pub search: bool,
}

impl OverlayFlags {
    pub fn contains(self, kind: OverlayKind) -> bool {
        match kind {
            OverlayKind::Palette => self.palette,
            OverlayKind::Help => self.help,
            OverlayKind::Settings => self.settings,
            OverlayKind::Search => self.search,
        }
    }

    pub fn any(self) -> bool {
        self.palette || self.help || self.settings || self.search
    }

    pub fn is_exclusive(self) -> bool {
        self.settings || self.search
    }

    /// Top of the Esc stack: exclusive first, then palette, then help.
    pub fn top(self) -> Option<OverlayKind> {
        if self.settings {
            Some(OverlayKind::Settings)
        } else if self.search {
            Some(OverlayKind::Search)
        } else if self.palette {
            Some(OverlayKind::Palette)
        } else if self.help {
            Some(OverlayKind::Help)
        } else {
            None
        }
    }

    pub fn open(&mut self, kind: OverlayKind) {
        match kind {
            OverlayKind::Settings => {
                self.palette = false;
                self.help = false;
                self.search = false;
                self.settings = true;
            }
            OverlayKind::Search => {
                self.palette = false;
                self.help = false;
                self.settings = false;
                self.search = true;
            }
            OverlayKind::Palette => {
                self.settings = false;
                self.search = false;
                self.palette = true;
            }
            OverlayKind::Help => {
                self.settings = false;
                self.search = false;
                self.help = true;
            }
        }
    }

    pub fn close(&mut self, kind: OverlayKind) {
        match kind {
            OverlayKind::Palette => self.palette = false,
            OverlayKind::Help => self.help = false,
            OverlayKind::Settings => self.settings = false,
            OverlayKind::Search => self.search = false,
        }
    }

    pub fn toggle(&mut self, kind: OverlayKind) {
        if self.contains(kind) {
            self.close(kind);
        } else {
            self.open(kind);
        }
    }

    pub fn pop(&mut self) -> Option<OverlayKind> {
        let top = self.top()?;
        self.close(top);
        Some(top)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_replaces_stack() {
        let mut f = OverlayFlags::default();
        f.open(OverlayKind::Palette);
        f.open(OverlayKind::Help);
        assert!(f.palette && f.help);
        assert!(!f.is_exclusive());
        assert_eq!(f.top(), Some(OverlayKind::Palette));

        f.open(OverlayKind::Settings);
        assert!(f.settings);
        assert!(!f.palette && !f.help && !f.search);
        assert!(f.is_exclusive());
        assert_eq!(f.top(), Some(OverlayKind::Settings));

        f.open(OverlayKind::Search);
        assert!(f.search && !f.settings);
        assert_eq!(f.top(), Some(OverlayKind::Search));

        f.open(OverlayKind::Palette);
        assert!(f.palette && !f.search && !f.settings);
        assert_eq!(f.top(), Some(OverlayKind::Palette));
    }

    #[test]
    fn help_and_palette_stack() {
        let mut f = OverlayFlags::default();
        f.open(OverlayKind::Help);
        f.open(OverlayKind::Palette);
        assert!(f.palette && f.help);
        assert_eq!(f.top(), Some(OverlayKind::Palette));
        assert_eq!(f.pop(), Some(OverlayKind::Palette));
        assert!(f.help && !f.palette);
        assert_eq!(f.pop(), Some(OverlayKind::Help));
        assert_eq!(f.pop(), None);
        assert!(!f.any());
    }

    #[test]
    fn toggle_closes_same_kind() {
        let mut f = OverlayFlags::default();
        f.toggle(OverlayKind::Settings);
        assert!(f.settings);
        f.toggle(OverlayKind::Settings);
        assert!(!f.settings);
        f.toggle(OverlayKind::Search);
        assert!(f.search);
        f.toggle(OverlayKind::Palette);
        assert!(f.palette && !f.search);
    }

    #[test]
    fn close_only_that_flag() {
        let mut f = OverlayFlags {
            palette: true,
            help: true,
            settings: false,
            search: false,
        };
        f.close(OverlayKind::Help);
        assert!(f.palette && !f.help);
        f.close(OverlayKind::Palette);
        assert!(!f.any());
        f.close(OverlayKind::Settings);
        assert!(!f.settings);
    }

    #[test]
    fn contains_and_default() {
        let f = OverlayFlags::default();
        assert!(!f.contains(OverlayKind::Palette));
        assert!(!f.contains(OverlayKind::Help));
        assert!(!f.contains(OverlayKind::Settings));
        assert!(!f.contains(OverlayKind::Search));
        assert_eq!(f.top(), None);
        assert!(!f.any());
        assert!(!f.is_exclusive());
        assert!(OverlayFlags {
            palette: true,
            ..OverlayFlags::default()
        }
        .any());
        assert!(OverlayFlags {
            help: true,
            ..OverlayFlags::default()
        }
        .any());
        assert!(OverlayFlags {
            settings: true,
            ..OverlayFlags::default()
        }
        .any());
        assert!(OverlayFlags {
            search: true,
            ..OverlayFlags::default()
        }
        .any());
    }

    #[test]
    fn settings_beats_search_on_top_if_both() {
        let f = OverlayFlags {
            palette: true,
            help: true,
            settings: true,
            search: true,
        };
        assert_eq!(f.top(), Some(OverlayKind::Settings));
        let search_only = OverlayFlags {
            palette: true,
            help: true,
            settings: false,
            search: true,
        };
        assert_eq!(search_only.top(), Some(OverlayKind::Search));
    }
}
