//! Directory tiles for models, MCP, skills, and git.

use crate::icons::{BrandIcon, ChromeGlyph};
use crate::widgets::{BadgeSpec, Tone};
use crate::Workspace;

/// One dashboard tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileSpec {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub badge: BadgeSpec,
}

/// All integration tiles for the workspace.
pub fn integration_tiles(ws: &Workspace) -> Vec<TileSpec> {
    let mut tiles = Vec::new();
    for model in &ws.models {
        tiles.push(TileSpec {
            id: format!("model:{model}"),
            title: model.clone(),
            subtitle: "provider".into(),
            icon: ChromeGlyph::Sparkle.mark().into(),
            badge: BadgeSpec::new(
                if *model == ws.model {
                    Tone::Accent
                } else {
                    Tone::Neutral
                },
                if *model == ws.model {
                    "active"
                } else {
                    "ready"
                },
            ),
        });
    }
    for m in &ws.mcp {
        let icon = BrandIcon::from_name(&m.name)
            .or_else(|| BrandIcon::from_name(&m.command))
            .map(|b| b.slug().to_string())
            .unwrap_or_else(|| ChromeGlyph::Plug.mark().into());
        tiles.push(TileSpec {
            id: format!("mcp:{}", m.name),
            title: m.name.clone(),
            subtitle: m.transport.clone(),
            icon,
            badge: BadgeSpec::new(Tone::Neutral, "configured"),
        });
    }
    for s in &ws.skills {
        tiles.push(TileSpec {
            id: format!("skill:{s}"),
            title: s.clone(),
            subtitle: "skill".into(),
            icon: ChromeGlyph::Sparkle.mark().into(),
            badge: BadgeSpec::new(Tone::Good, "installed"),
        });
    }
    for (i, path) in ws.worktrees.iter().enumerate() {
        tiles.push(TileSpec {
            id: format!("git:{i}"),
            title: path.clone(),
            subtitle: "worktree".into(),
            icon: ChromeGlyph::Git.mark().into(),
            badge: BadgeSpec::new(Tone::Neutral, "git"),
        });
    }
    tiles
}

pub fn filter_tiles<'a>(tiles: &'a [TileSpec], query: &str) -> Vec<&'a TileSpec> {
    if query.is_empty() {
        return tiles.iter().collect();
    }
    let q = query.to_ascii_lowercase();
    tiles
        .iter()
        .filter(|t| {
            t.title.to_ascii_lowercase().contains(&q)
                || t.subtitle.to_ascii_lowercase().contains(&q)
                || t.id.to_ascii_lowercase().contains(&q)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::McpRow;

    #[test]
    fn tiles_include_active_model() {
        let ws = Workspace::new("p", "grok");
        let tiles = integration_tiles(&ws);
        assert!(tiles.iter().any(|t| t.id == "model:grok"));
        assert_eq!(
            tiles
                .iter()
                .find(|t| t.id == "model:grok")
                .unwrap()
                .badge
                .text,
            "active"
        );
    }

    #[test]
    fn filter_tiles_narrows() {
        let mut ws = Workspace::new("p", "grok");
        ws.mcp.push(McpRow {
            name: "linear".into(),
            command: "npx".into(),
            transport: "stdio".into(),
        });
        let tiles = integration_tiles(&ws);
        let hit = filter_tiles(&tiles, "lin");
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].title, "linear");
        assert!(filter_tiles(&tiles, "").len() >= 2);
    }
}
