//! Glass chrome tokens for the desktop shell.

use gpui::{hsla, point, px, BoxShadow, Hsla, Pixels};

pub struct Theme;

impl Theme {
    pub fn glass() -> Hsla {
        hsla(0.64, 0.16, 0.10, 0.52)
    }

    pub fn glass_strong() -> Hsla {
        hsla(0.64, 0.18, 0.12, 0.68)
    }

    pub fn ink() -> Hsla {
        hsla(0.64, 0.22, 0.06, 0.35)
    }

    pub fn hairline() -> Hsla {
        hsla(0.0, 0.0, 1.0, 0.10)
    }

    pub fn hairline_bright() -> Hsla {
        hsla(0.0, 0.0, 1.0, 0.18)
    }

    pub fn text() -> Hsla {
        hsla(0.62, 0.08, 0.92, 0.94)
    }

    pub fn muted() -> Hsla {
        hsla(0.62, 0.08, 0.72, 0.72)
    }

    pub fn accent() -> Hsla {
        hsla(0.58, 0.72, 0.62, 0.95)
    }

    pub fn good() -> Hsla {
        hsla(0.38, 0.55, 0.58, 0.95)
    }

    /// Ghost-button family (`hsla(0, 0, 1, 0.07)`), slightly brighter.
    pub fn send_bg() -> Hsla {
        hsla(0.0, 0.0, 1.0, 0.11)
    }

    pub fn danger() -> Hsla {
        hsla(0.02, 0.68, 0.58, 0.95)
    }

    pub fn panel_radius() -> Pixels {
        px(12.)
    }

    pub fn shadow() -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: hsla(0.64, 0.30, 0.04, 0.45),
                offset: point(px(0.), px(10.)),
                blur_radius: px(28.),
                spread_radius: px(-4.),
            },
            BoxShadow {
                color: hsla(0.0, 0.0, 1.0, 0.04),
                offset: point(px(0.), px(1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
            },
        ]
    }
}
