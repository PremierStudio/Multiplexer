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
}

impl ChromeGlyph {
    pub fn all() -> [ChromeGlyph; 22] {
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
        }
    }
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
        assert_eq!(ChromeGlyph::all().len(), 22);
        assert_eq!(ChromeGlyph::Palette.mark(), "⌘");
        assert!(!ChromeGlyph::Chat.mark().is_empty());
        for g in ChromeGlyph::all() {
            assert!(!g.mark().is_empty());
        }
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
