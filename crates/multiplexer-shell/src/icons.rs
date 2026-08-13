//! Chrome glyphs and brand-icon name mapping (dashboardicons.com).

/// First-party chrome glyph. Desktop draws these as text marks or paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeGlyph {
    Chat,
    Agent,
    Folder,
    Git,
    Terminal,
    Cpu,
    Plug,
    Flag,
    Search,
    Plus,
    Close,
    Chevron,
    Play,
    Stop,
    Copy,
    Settings,
    Sparkle,
    Layout,
    Browser,
    Diff,
    Activity,
    Palette,
    Session,
    Skills,
}

impl ChromeGlyph {
    pub fn all() -> [ChromeGlyph; 24] {
        [
            Self::Chat,
            Self::Agent,
            Self::Folder,
            Self::Git,
            Self::Terminal,
            Self::Cpu,
            Self::Plug,
            Self::Flag,
            Self::Search,
            Self::Plus,
            Self::Close,
            Self::Chevron,
            Self::Play,
            Self::Stop,
            Self::Copy,
            Self::Settings,
            Self::Sparkle,
            Self::Layout,
            Self::Browser,
            Self::Diff,
            Self::Activity,
            Self::Palette,
            Self::Session,
            Self::Skills,
        ]
    }

    pub fn mark(self) -> &'static str {
        match self {
            Self::Chat => "☰",
            Self::Agent => "⚡",
            Self::Folder => "▤",
            Self::Git => "⎇",
            Self::Terminal => ">_",
            Self::Cpu => "▣",
            Self::Plug => "⬡",
            Self::Flag => "⚑",
            Self::Search => "⌕",
            Self::Plus => "+",
            Self::Close => "×",
            Self::Chevron => "›",
            Self::Play => "▶",
            Self::Stop => "■",
            Self::Copy => "⧉",
            Self::Settings => "⚙",
            Self::Sparkle => "✦",
            Self::Layout => "▦",
            Self::Browser => "◎",
            Self::Diff => "±",
            Self::Activity => "●",
            Self::Palette => "⌘",
            Self::Session => "◉",
            Self::Skills => "✦",
        }
    }

    /// Vendored Lucide SVG path (ISC). Desktop paints these via GPUI `svg()`.
    pub fn icon_file(self) -> &'static str {
        match self {
            Self::Chat => "icons/message-square.svg",
            Self::Agent => "icons/zap.svg",
            Self::Folder => "icons/folder.svg",
            Self::Git => "icons/git-branch.svg",
            Self::Terminal => "icons/square-terminal.svg",
            Self::Cpu => "icons/cpu.svg",
            Self::Plug => "icons/unplug.svg",
            Self::Flag => "icons/flag.svg",
            Self::Search => "icons/search.svg",
            Self::Plus => "icons/plus.svg",
            Self::Close => "icons/x.svg",
            Self::Chevron => "icons/chevron-right.svg",
            Self::Play => "icons/play.svg",
            Self::Stop => "icons/square.svg",
            Self::Copy => "icons/copy.svg",
            Self::Settings => "icons/settings.svg",
            Self::Sparkle => "icons/sparkles.svg",
            Self::Layout => "icons/panel-left.svg",
            Self::Browser => "icons/globe.svg",
            Self::Diff => "icons/file-diff.svg",
            Self::Activity => "icons/activity.svg",
            Self::Palette => "icons/command.svg",
            Self::Session => "icons/circle-user.svg",
            Self::Skills => "icons/book-open.svg",
        }
    }
}

/// GPUI `svg()` keeps an alpha mask. Lucide uses `currentColor`, which
/// usvg leaves unpainted. Force a real stroke so the tint can show.
pub fn lucide_for_gpui(svg: &str) -> String {
    svg.replace("currentColor", "#000000")
}

/// Vendored dashboard-icons filename (png, dark-UI / -light variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandIcon {
    Github,
    Git,
    Rust,
    Docker,
    Cloudflare,
    Slack,
    Windows,
    Nodejs,
    Linear,
    Notion,
}

impl BrandIcon {
    pub fn all() -> [BrandIcon; 10] {
        [
            Self::Github,
            Self::Git,
            Self::Rust,
            Self::Docker,
            Self::Cloudflare,
            Self::Slack,
            Self::Windows,
            Self::Nodejs,
            Self::Linear,
            Self::Notion,
        ]
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Github => "github-light",
            Self::Git => "git",
            Self::Rust => "rust",
            Self::Docker => "docker",
            Self::Cloudflare => "cloudflare",
            Self::Slack => "slack",
            Self::Windows => "windows",
            Self::Nodejs => "nodejs",
            Self::Linear => "linear",
            Self::Notion => "notion",
        }
    }

    pub fn asset_path(self) -> String {
        format!("assets/brands/{}.png", self.slug())
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let n = name.to_ascii_lowercase();
        if n.contains("github") || n == "gh" {
            Some(Self::Github)
        } else if n.contains("cloudflare") || n.contains("wrangler") {
            Some(Self::Cloudflare)
        } else if n.contains("docker") {
            Some(Self::Docker)
        } else if n.contains("slack") {
            Some(Self::Slack)
        } else if n.contains("linear") {
            Some(Self::Linear)
        } else if n.contains("notion") {
            Some(Self::Notion)
        } else if n.contains("node") || n.contains("npx") {
            Some(Self::Nodejs)
        } else if n.contains("windows") {
            Some(Self::Windows)
        } else if n.contains("rust") || n.contains("cargo") {
            Some(Self::Rust)
        } else if n == "git" || n.contains("git ") {
            Some(Self::Git)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_from_name_github() {
        assert_eq!(BrandIcon::from_name("github"), Some(BrandIcon::Github));
        assert_eq!(
            BrandIcon::from_name("npx @linear/mcp"),
            Some(BrandIcon::Linear)
        );
        assert_eq!(BrandIcon::from_name("unknown-tool"), None);
        assert_ne!(
            BrandIcon::from_name("docker"),
            BrandIcon::from_name("slack")
        );
    }

    #[test]
    fn chrome_glyph_catalog() {
        assert_eq!(ChromeGlyph::all().len(), 24);
        assert_eq!(ChromeGlyph::Palette.mark(), "⌘");
        assert!(!ChromeGlyph::Chat.mark().is_empty());
        let mut files = std::collections::BTreeSet::new();
        for g in ChromeGlyph::all() {
            assert!(!g.mark().is_empty());
            let file = g.icon_file();
            assert!(file.starts_with("icons/"), "{file}");
            assert!(file.ends_with(".svg"), "{file}");
            assert!(files.insert(file), "duplicate icon {file}");
        }
        assert_eq!(ChromeGlyph::Chat.icon_file(), "icons/message-square.svg");
        assert_ne!(
            ChromeGlyph::Chat.icon_file(),
            ChromeGlyph::Folder.icon_file()
        );
        let painted = lucide_for_gpui("stroke=\"currentColor\" fill=\"none\"");
        assert!(painted.contains("#000000"));
        assert!(!painted.contains("currentColor"));
        assert_ne!(painted, "stroke=\"currentColor\" fill=\"none\"");
    }

    #[test]
    fn brand_asset_path_uses_slug() {
        assert_eq!(
            BrandIcon::Github.asset_path(),
            "assets/brands/github-light.png"
        );
        assert_eq!(BrandIcon::all().len(), 10);
    }
}
