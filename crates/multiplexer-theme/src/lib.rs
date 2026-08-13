//! Headless theme tokens for Multiplexer.
//!
//! No GPUI types. The desktop maps [`HslaTuple`] through `hsla(h, s, l, a)`.

mod tokens;

pub use tokens::{
    space, Density, Elevation, HslaTuple, Motion, Radius, ThemeMode, ThemeTokens, TypeScale,
    ELEVATION_MAX,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_page_is_warm_near_black() {
        let t = ThemeTokens::dark();
        assert!(t.bg.l < 0.12, "page light {}", t.bg.l);
        assert!(t.ink.a > 0.95);
        assert!(t.accent.h < 0.12, "accent is warm orange, not cyan");
        assert!(t.text.l > 0.70);
        assert_eq!(t.mode, ThemeMode::Dark);
        assert!(t.glass_ultra.a <= t.glass.a);
    }

    #[test]
    fn light_differs_from_dark() {
        let d = ThemeTokens::dark();
        let l = ThemeTokens::light();
        assert_ne!(d.ink, l.ink);
        assert_ne!(d.text, l.text);
        assert_ne!(d.surface, l.surface);
        assert_eq!(l.mode, ThemeMode::Light);
        assert!(l.text.l < 0.35, "light text is dark");
        assert!(d.text.l > 0.70, "dark text is light");
    }

    #[test]
    fn density_compact_shrinks_space() {
        let cozy = ThemeTokens::dark().with_density(Density::Comfortable);
        let compact = ThemeTokens::dark().with_density(Density::Compact);
        assert!(compact.space_1 < cozy.space_1);
        assert!(compact.space_2 < cozy.space_2);
        assert!(compact.space_3 < cozy.space_3);
        assert_eq!(space(Density::Comfortable, 2), cozy.space_2);
        assert_eq!(space(Density::Compact, 2), compact.space_2);
    }

    #[test]
    fn elevation_monotonic_alpha() {
        let t = ThemeTokens::dark();
        let mut prev = 0.0;
        for e in Elevation::all() {
            let g = t.glass_at(e);
            assert!(g.a + f32::EPSILON >= prev, "elev {e:?} alpha {}", g.a);
            prev = g.a;
            assert!((0.0..=1.0).contains(&g.a));
        }
        assert_eq!(Elevation::all().len(), ELEVATION_MAX + 1);
    }

    #[test]
    fn accent_not_equal_good() {
        let t = ThemeTokens::dark();
        assert_ne!(t.accent, t.good);
        assert_ne!(t.accent, t.danger);
        assert_ne!(t.good, t.warn);
        assert_ne!(t.danger, t.warn);
        assert!(t.accent.s > 0.4);
    }

    #[test]
    fn token_channels_are_unit_interval() {
        for t in [ThemeTokens::dark(), ThemeTokens::light()] {
            for c in t.all_colors() {
                assert!((0.0..=1.0).contains(&c.h), "h {}", c.h);
                assert!((0.0..=1.0).contains(&c.s), "s {}", c.s);
                assert!((0.0..=1.0).contains(&c.l), "l {}", c.l);
                assert!((0.0..=1.0).contains(&c.a), "a {}", c.a);
            }
        }
    }

    #[test]
    fn type_scale_is_strictly_increasing() {
        let steps = TypeScale::all();
        for pair in steps.windows(2) {
            assert!(pair[0] < pair[1], "{pair:?}");
        }
        assert_eq!(TypeScale::UI, 13.0);
    }

    #[test]
    fn radius_and_motion_have_named_steps() {
        assert_eq!(
            [Radius::XS, Radius::SM, Radius::MD, Radius::LG, Radius::XL],
            [4.0, 8.0, 12.0, 16.0, 22.0]
        );
        assert_eq!(
            [Motion::FAST.ms, Motion::NORMAL.ms, Motion::SLOW.ms],
            [90, 160, 240]
        );
    }

    #[test]
    fn default_is_dark_comfortable() {
        let t = ThemeTokens::default();
        assert_eq!(t.mode, ThemeMode::Dark);
        assert_eq!(t.density, Density::Comfortable);
        assert_eq!(t, ThemeTokens::dark());
    }
}
