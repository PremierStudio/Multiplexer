//! GPUI adapter over [`multiplexer_theme::ThemeTokens`].

use gpui::{hsla, point, px, BoxShadow, Hsla, Pixels};
use multiplexer_theme::{HslaTuple, ThemeTokens};

pub struct Theme;

impl Theme {
    pub fn tokens() -> ThemeTokens {
        ThemeTokens::dark()
    }

    fn c(t: HslaTuple) -> Hsla {
        hsla(t.h, t.s, t.l, t.a)
    }

    pub fn glass() -> Hsla {
        Self::c(Self::tokens().glass)
    }
    pub fn glass_strong() -> Hsla {
        Self::c(Self::tokens().glass_strong)
    }
    pub fn glass_ultra() -> Hsla {
        Self::c(Self::tokens().glass_ultra)
    }
    pub fn ink() -> Hsla {
        Self::c(Self::tokens().ink)
    }
    pub fn hairline() -> Hsla {
        Self::c(Self::tokens().hairline)
    }
    pub fn hairline_bright() -> Hsla {
        Self::c(Self::tokens().hairline_bright)
    }
    pub fn text() -> Hsla {
        Self::c(Self::tokens().text)
    }
    pub fn muted() -> Hsla {
        Self::c(Self::tokens().text_muted)
    }
    pub fn faint() -> Hsla {
        Self::c(Self::tokens().text_faint)
    }
    pub fn accent() -> Hsla {
        Self::c(Self::tokens().accent)
    }
    pub fn accent_muted() -> Hsla {
        Self::c(Self::tokens().accent_muted)
    }
    pub fn good() -> Hsla {
        Self::c(Self::tokens().good)
    }
    pub fn warn() -> Hsla {
        Self::c(Self::tokens().warn)
    }
    pub fn send_bg() -> Hsla {
        Self::c(Self::tokens().accent_muted)
    }
    pub fn danger() -> Hsla {
        Self::c(Self::tokens().danger)
    }
    pub fn selection() -> Hsla {
        Self::c(Self::tokens().selection)
    }
    pub fn surface() -> Hsla {
        Self::c(Self::tokens().surface)
    }
    pub fn panel_radius() -> Pixels {
        px(multiplexer_theme::Radius::MD)
    }
    pub fn shadow() -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: hsla(0.64, 0.30, 0.04, 0.38),
                offset: point(px(0.), px(12.)),
                blur_radius: px(32.),
                spread_radius: px(-6.),
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 1.0, 0.07),
                offset: point(px(0.), px(1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_glass_is_translucent() {
        let g = Theme::glass();
        assert!(g.a < 0.55);
        assert!(Theme::glass_ultra().a < g.a);
    }
}
