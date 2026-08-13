//! GPUI adapter over [`multiplexer_theme::ThemeTokens`].

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{
    hsla, point, px, size, Bounds, BoxShadow, Hsla, Pixels, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use multiplexer_theme::{Density, HslaTuple, ThemeMode, ThemeTokens, TypeScale};

static THEME_MODE: AtomicU8 = AtomicU8::new(0);
static THEME_DENSITY: AtomicU8 = AtomicU8::new(0);

pub struct Theme;

impl Theme {
    pub fn set_mode(mode: ThemeMode) {
        THEME_MODE.store(
            match mode {
                ThemeMode::Dark => 0,
                ThemeMode::Light => 1,
            },
            Ordering::Relaxed,
        );
    }

    pub fn set_density(density: Density) {
        THEME_DENSITY.store(
            match density {
                Density::Comfortable => 0,
                Density::Compact => 1,
            },
            Ordering::Relaxed,
        );
    }

    pub fn tokens() -> ThemeTokens {
        match THEME_MODE.load(Ordering::Relaxed) {
            1 => ThemeTokens::light(),
            _ => ThemeTokens::dark(),
        }
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
    pub fn text_ui() -> Pixels {
        px(TypeScale::UI)
    }
    pub fn text_caption() -> Pixels {
        px(TypeScale::CAPTION)
    }
    pub fn text_body() -> Pixels {
        px(TypeScale::BODY)
    }
    pub fn row_height() -> Pixels {
        match THEME_DENSITY.load(Ordering::Relaxed) {
            1 => px(44.0),
            _ => px(56.0),
        }
    }
    pub fn icon_size() -> Pixels {
        px(32.0)
    }

    /// Native caption contract. `appears_transparent` stays false.
    pub fn window_options(bounds: Bounds<Pixels>) -> WindowOptions {
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_background: WindowBackgroundAppearance::Blurred,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            window_min_size: Some(size(px(920.0), px(620.0))),
            titlebar: Some(TitlebarOptions {
                title: Some("Multiplexer".into()),
                appears_transparent: false,
                traffic_light_position: None,
            }),
            ..Default::default()
        }
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
    use multiplexer_theme::TypeScale;

    #[test]
    fn adapter_glass_is_translucent() {
        let g = Theme::glass();
        assert!(g.a < 0.55);
        assert!(Theme::glass_ultra().a < g.a);
    }

    #[test]
    fn adapter_type_scale_matches_tokens() {
        assert_eq!(Theme::text_ui(), px(TypeScale::UI));
        assert_eq!(Theme::text_caption(), px(TypeScale::CAPTION));
        assert_eq!(Theme::text_body(), px(TypeScale::BODY));
        Theme::set_density(Density::Comfortable);
        assert_eq!(Theme::row_height(), px(56.0));
        Theme::set_density(Density::Compact);
        assert_eq!(Theme::row_height(), px(44.0));
        Theme::set_density(Density::Comfortable);
        assert_eq!(Theme::icon_size(), px(32.0));
        Theme::set_mode(ThemeMode::Light);
        assert_eq!(Theme::tokens().mode, ThemeMode::Light);
        Theme::set_mode(ThemeMode::Dark);
        assert_eq!(Theme::tokens().mode, ThemeMode::Dark);
    }

    #[test]
    fn window_options_keep_native_caption() {
        let bounds = Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1360.0), px(860.0)),
        };
        let opts = Theme::window_options(bounds);
        let bar = opts.titlebar.expect("native titlebar");
        assert!(!bar.appears_transparent);
        assert!(opts.is_movable);
        assert!(opts.is_resizable);
        assert!(opts.is_minimizable);
        assert_eq!(opts.window_background, WindowBackgroundAppearance::Blurred);
    }
}
