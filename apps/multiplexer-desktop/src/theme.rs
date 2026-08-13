//! GPUI adapter over [`multiplexer_theme::ThemeTokens`].

use std::sync::atomic::{AtomicU8, Ordering};

use gpui::{
    hsla, point, px, size, Bounds, BoxShadow, Hsla, Pixels, TitlebarOptions,
    WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use multiplexer_theme::{Density, HslaTuple, ThemeMode, ThemeTokens, TypeScale};

static THEME_MODE: AtomicU8 = AtomicU8::new(0);
static THEME_DENSITY: AtomicU8 = AtomicU8::new(0);
static THEME_CONTRAST: AtomicU8 = AtomicU8::new(0);
static THEME_SCALE: AtomicU8 = AtomicU8::new(100);

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

    pub fn set_high_contrast(on: bool) {
        THEME_CONTRAST.store(if on { 1 } else { 0 }, Ordering::Relaxed);
    }

    pub fn set_ui_scale(scale: u16) {
        THEME_SCALE.store(scale.clamp(100, 200) as u8, Ordering::Relaxed);
    }

    pub fn high_contrast() -> bool {
        THEME_CONTRAST.load(Ordering::Relaxed) == 1
    }

    pub fn ui_scale() -> f32 {
        f32::from(THEME_SCALE.load(Ordering::Relaxed)) / 100.0
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
        Self::c(Self::tokens().accent)
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
    pub fn transparent() -> Hsla {
        Self::c(Self::tokens().hairline.with_alpha(0.0))
    }
    pub fn wash() -> Hsla {
        Self::c(Self::tokens().hairline_bright.with_alpha(0.08))
    }
    pub fn wash_soft() -> Hsla {
        Self::c(Self::tokens().hairline.with_alpha(0.05))
    }
    pub fn overlay_scrim() -> Hsla {
        let t = Self::tokens();
        Self::c(
            t.ink
                .with_alpha(if Self::high_contrast() { 0.55 } else { 0.45 }),
        )
    }
    pub fn hover_fill() -> Hsla {
        Self::c(Self::tokens().accent_muted.with_alpha(0.28))
    }
    pub fn hover_strong() -> Hsla {
        Self::c(Self::tokens().accent_muted.with_alpha(0.40))
    }
    pub fn bubble_user() -> Hsla {
        Self::c(Self::tokens().selection.with_alpha(0.55))
    }
    pub fn bubble_assistant() -> Hsla {
        Self::wash()
    }
    pub fn reminder_fill() -> Hsla {
        Self::c(Self::tokens().warn.with_alpha(0.22))
    }
    pub fn approval_fill() -> Hsla {
        Self::c(Self::tokens().danger.with_alpha(0.22))
    }
    pub fn toast_fill(kind: multiplexer_shell::NoticeKind) -> Hsla {
        let t = Self::tokens();
        let base = match kind {
            multiplexer_shell::NoticeKind::Good => t.good,
            multiplexer_shell::NoticeKind::Warn => t.warn,
            multiplexer_shell::NoticeKind::Danger => t.danger,
            _ => t.accent_muted,
        };
        Self::c(base.with_alpha(0.28))
    }
    pub fn ghost_fill() -> Hsla {
        Self::c(Self::tokens().hairline_bright.with_alpha(0.07))
    }
    pub fn title_height() -> Pixels {
        px(48.0 * Self::ui_scale())
    }
    pub fn rail_width() -> Pixels {
        px(48.0 * Self::ui_scale())
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
        let scale = Self::ui_scale();
        match THEME_DENSITY.load(Ordering::Relaxed) {
            1 => px(48.0 * scale),
            _ => px(56.0 * scale),
        }
    }
    pub fn icon_size() -> Pixels {
        px(28.0)
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
        let t = Self::tokens();
        vec![
            BoxShadow {
                color: Self::c(t.ink.with_alpha(0.38)),
                offset: point(px(0.), px(12.)),
                blur_radius: px(32.),
                spread_radius: px(-6.),
            },
            BoxShadow {
                color: Self::c(t.hairline.with_alpha(0.07)),
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
    fn adapter_page_is_opaque_product_surface() {
        let g = Theme::glass();
        assert!(g.a > 0.90);
        assert!(Theme::accent().h < 0.12);
        Theme::set_mode(multiplexer_theme::ThemeMode::Dark);
        assert!(Theme::tokens().bg.l < 0.12);
    }

    #[test]
    fn adapter_type_scale_matches_tokens() {
        assert_eq!(Theme::text_ui(), px(TypeScale::UI));
        assert_eq!(Theme::text_caption(), px(TypeScale::CAPTION));
        assert_eq!(Theme::text_body(), px(TypeScale::BODY));
        Theme::set_density(Density::Comfortable);
        assert_eq!(Theme::row_height(), px(56.0));
        Theme::set_density(Density::Compact);
        Theme::set_ui_scale(100);
        assert_eq!(Theme::row_height(), px(48.0));
        Theme::set_density(Density::Comfortable);
        assert_eq!(Theme::icon_size(), px(28.0));
        Theme::set_mode(ThemeMode::Light);
        assert_eq!(Theme::tokens().mode, ThemeMode::Light);
        assert!(Theme::tokens().text.l < 0.35);
        Theme::set_mode(ThemeMode::Dark);
        assert_eq!(Theme::tokens().mode, ThemeMode::Dark);
        Theme::set_high_contrast(true);
        assert!(Theme::high_contrast());
        Theme::set_high_contrast(false);
        assert!(!Theme::high_contrast());
        assert!(Theme::glass().a > 0.90);
        assert!(Theme::approval_fill().a <= 0.55);
        assert!(Theme::toast_fill(multiplexer_shell::NoticeKind::Danger).a <= 0.55);
        assert!(Theme::send_bg().h < 0.12);
        assert!(Theme::bubble_user().a > 0.0);
        assert!(Theme::bubble_assistant().a > 0.0);
        let _ = Theme::ghost_fill();
        let _ = Theme::title_height();
        let _ = Theme::panel_radius();
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
