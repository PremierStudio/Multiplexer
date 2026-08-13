//! Embedded Lucide icons (ISC) and Geist fonts (SIL OFL 1.1).

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

/// File-backed and `include_bytes` asset table for GPUI `svg()` and fonts.
pub struct DesktopAssets;

pub const UI_FONT: &str = "Geist";
pub const MONO_FONT: &str = "Geist Mono";

impl DesktopAssets {
    pub fn font_bytes() -> Vec<Cow<'static, [u8]>> {
        vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Regular.ttf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-Medium.ttf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/Geist-SemiBold.ttf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/GeistMono-Regular.ttf").as_slice()),
            Cow::Borrowed(include_bytes!("../assets/fonts/GeistMono-Medium.ttf").as_slice()),
        ]
    }

    pub fn bytes(path: &str) -> Option<&'static [u8]> {
        Some(match path {
            "icons/activity.svg" => include_bytes!("../assets/icons/activity.svg").as_slice(),
            "icons/book-open.svg" => include_bytes!("../assets/icons/book-open.svg").as_slice(),
            "icons/chevron-right.svg" => {
                include_bytes!("../assets/icons/chevron-right.svg").as_slice()
            }
            "icons/circle-user.svg" => include_bytes!("../assets/icons/circle-user.svg").as_slice(),
            "icons/command.svg" => include_bytes!("../assets/icons/command.svg").as_slice(),
            "icons/copy.svg" => include_bytes!("../assets/icons/copy.svg").as_slice(),
            "icons/cpu.svg" => include_bytes!("../assets/icons/cpu.svg").as_slice(),
            "icons/file-diff.svg" => include_bytes!("../assets/icons/file-diff.svg").as_slice(),
            "icons/flag.svg" => include_bytes!("../assets/icons/flag.svg").as_slice(),
            "icons/folder.svg" => include_bytes!("../assets/icons/folder.svg").as_slice(),
            "icons/git-branch.svg" => include_bytes!("../assets/icons/git-branch.svg").as_slice(),
            "icons/globe.svg" => include_bytes!("../assets/icons/globe.svg").as_slice(),
            "icons/message-square.svg" => {
                include_bytes!("../assets/icons/message-square.svg").as_slice()
            }
            "icons/panel-left.svg" => include_bytes!("../assets/icons/panel-left.svg").as_slice(),
            "icons/play.svg" => include_bytes!("../assets/icons/play.svg").as_slice(),
            "icons/plus.svg" => include_bytes!("../assets/icons/plus.svg").as_slice(),
            "icons/search.svg" => include_bytes!("../assets/icons/search.svg").as_slice(),
            "icons/settings.svg" => include_bytes!("../assets/icons/settings.svg").as_slice(),
            "icons/sparkles.svg" => include_bytes!("../assets/icons/sparkles.svg").as_slice(),
            "icons/square-terminal.svg" => {
                include_bytes!("../assets/icons/square-terminal.svg").as_slice()
            }
            "icons/square.svg" => include_bytes!("../assets/icons/square.svg").as_slice(),
            "icons/unplug.svg" => include_bytes!("../assets/icons/unplug.svg").as_slice(),
            "icons/x.svg" => include_bytes!("../assets/icons/x.svg").as_slice(),
            "icons/zap.svg" => include_bytes!("../assets/icons/zap.svg").as_slice(),
            _ => return None,
        })
    }
}

impl AssetSource for DesktopAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(DesktopAssets::bytes(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let prefix = if path.is_empty() {
            "icons/"
        } else if path.ends_with('/') {
            path
        } else {
            return Ok(Vec::new());
        };
        Ok(ICON_FILES
            .iter()
            .copied()
            .filter(|p| p.starts_with(prefix))
            .map(SharedString::from)
            .collect())
    }
}

const ICON_FILES: &[&str] = &[
    "icons/activity.svg",
    "icons/book-open.svg",
    "icons/chevron-right.svg",
    "icons/circle-user.svg",
    "icons/command.svg",
    "icons/copy.svg",
    "icons/cpu.svg",
    "icons/file-diff.svg",
    "icons/folder.svg",
    "icons/flag.svg",
    "icons/git-branch.svg",
    "icons/globe.svg",
    "icons/message-square.svg",
    "icons/panel-left.svg",
    "icons/play.svg",
    "icons/plus.svg",
    "icons/search.svg",
    "icons/settings.svg",
    "icons/sparkles.svg",
    "icons/square-terminal.svg",
    "icons/square.svg",
    "icons/unplug.svg",
    "icons/x.svg",
    "icons/zap.svg",
];

#[cfg(test)]
mod tests {
    use super::*;
    use multiplexer_shell::ChromeGlyph;

    #[test]
    fn every_chrome_glyph_is_embedded() {
        for g in ChromeGlyph::all() {
            let path = g.icon_file();
            let bytes = DesktopAssets::bytes(path).unwrap_or_else(|| panic!("missing {path}"));
            assert!(bytes.starts_with(b"<svg"), "{path} is not svg");
            assert!(bytes.len() > 80, "{path} too small");
        }
        assert!(DesktopAssets::bytes("icons/nope.svg").is_none());
        assert_ne!(
            DesktopAssets::bytes("icons/zap.svg").unwrap().len(),
            DesktopAssets::bytes("icons/folder.svg").unwrap().len()
        );
    }

    #[test]
    fn fonts_are_true_ttf() {
        for blob in DesktopAssets::font_bytes() {
            assert!(blob.len() > 1000);
            assert_eq!(&blob[..4], &[0x00, 0x01, 0x00, 0x00]);
        }
        assert_eq!(UI_FONT, "Geist");
        assert_eq!(MONO_FONT, "Geist Mono");
        assert_ne!(UI_FONT, MONO_FONT);
    }
}
