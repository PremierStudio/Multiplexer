//! Token tables. Values are HSLA in 0..=1 (GPUI `hsla` units).

/// Highest elevation index (0..=ELEVATION_MAX).
pub const ELEVATION_MAX: usize = 4;

/// Hue/sat/light/alpha in `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HslaTuple {
    pub h: f32,
    pub s: f32,
    pub l: f32,
    pub a: f32,
}

impl HslaTuple {
    pub const fn new(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self { h, s, l, a }
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self {
            a: a.clamp(0.0, 1.0),
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Density {
    Comfortable,
    Compact,
}

/// Panel lift. 0 is the page, 4 is a floating overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Elevation {
    Base = 0,
    Sunken = 1,
    Raised = 2,
    Overlay = 3,
    Float = 4,
}

impl Elevation {
    pub fn all() -> [Elevation; 5] {
        [
            Self::Base,
            Self::Sunken,
            Self::Raised,
            Self::Overlay,
            Self::Float,
        ]
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// Corner radius in CSS-like pixels.
pub struct Radius;

impl Radius {
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 8.0;
    pub const MD: f32 = 12.0;
    pub const LG: f32 = 16.0;
    pub const XL: f32 = 22.0;
}

/// Type sizes in CSS-like pixels.
pub struct TypeScale;

impl TypeScale {
    pub const CAPTION: f32 = 11.0;
    pub const SMALL: f32 = 12.0;
    pub const UI: f32 = 13.0;
    pub const BODY: f32 = 14.0;
    pub const TITLE: f32 = 16.0;
    pub const DISPLAY: f32 = 20.0;
    pub const HERO: f32 = 24.0;

    pub fn all() -> [f32; 7] {
        [
            Self::CAPTION,
            Self::SMALL,
            Self::UI,
            Self::BODY,
            Self::TITLE,
            Self::DISPLAY,
            Self::HERO,
        ]
    }
}

/// Motion durations. Easing names only; GPUI wiring is later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Motion {
    pub ms: u16,
    pub easing: &'static str,
}

impl Motion {
    pub const FAST: Motion = Motion {
        ms: 90,
        easing: "ease-out",
    };
    pub const NORMAL: Motion = Motion {
        ms: 160,
        easing: "ease-in-out",
    };
    pub const SLOW: Motion = Motion {
        ms: 240,
        easing: "ease-in-out",
    };
}

/// Space step 1..=6 for a density. Step 0 is 0.
pub fn space(density: Density, step: u8) -> f32 {
    let base = match density {
        Density::Comfortable => [0.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0],
        Density::Compact => [0.0, 2.0, 6.0, 8.0, 12.0, 16.0, 24.0],
    };
    let i = usize::from(step.min(6));
    base[i]
}

/// Full token set for one mode + density.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeTokens {
    pub mode: ThemeMode,
    pub density: Density,
    pub bg: HslaTuple,
    pub surface: HslaTuple,
    pub surface_raised: HslaTuple,
    pub glass: HslaTuple,
    pub glass_strong: HslaTuple,
    pub glass_ultra: HslaTuple,
    pub ink: HslaTuple,
    pub text: HslaTuple,
    pub text_muted: HslaTuple,
    pub text_faint: HslaTuple,
    pub accent: HslaTuple,
    pub accent_muted: HslaTuple,
    pub good: HslaTuple,
    pub warn: HslaTuple,
    pub danger: HslaTuple,
    pub hairline: HslaTuple,
    pub hairline_bright: HslaTuple,
    pub selection: HslaTuple,
    pub focus_ring: HslaTuple,
    pub space_1: f32,
    pub space_2: f32,
    pub space_3: f32,
    pub space_4: f32,
    pub space_5: f32,
    pub space_6: f32,
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self::dark()
    }
}

impl ThemeTokens {
    /// Product dark: warm near-black (OpenChamber / Flexoki), not purple glass.
    pub fn dark() -> Self {
        Self::from_mode(ThemeMode::Dark, Density::Comfortable)
    }

    pub fn light() -> Self {
        Self::from_mode(ThemeMode::Light, Density::Comfortable)
    }

    pub fn with_density(mut self, density: Density) -> Self {
        self.density = density;
        self.apply_density();
        self
    }

    pub fn glass_at(&self, elevation: Elevation) -> HslaTuple {
        let base = self.glass.a;
        let delta = 0.07 * elevation.index() as f32;
        self.glass.with_alpha((base + delta).min(0.78))
    }

    pub fn all_colors(&self) -> [HslaTuple; 19] {
        [
            self.bg,
            self.surface,
            self.surface_raised,
            self.glass,
            self.glass_strong,
            self.glass_ultra,
            self.ink,
            self.text,
            self.text_muted,
            self.text_faint,
            self.accent,
            self.accent_muted,
            self.good,
            self.warn,
            self.danger,
            self.hairline,
            self.hairline_bright,
            self.selection,
            self.focus_ring,
        ]
    }

    fn apply_density(&mut self) {
        self.space_1 = space(self.density, 1);
        self.space_2 = space(self.density, 2);
        self.space_3 = space(self.density, 3);
        self.space_4 = space(self.density, 4);
        self.space_5 = space(self.density, 5);
        self.space_6 = space(self.density, 6);
    }

    fn from_mode(mode: ThemeMode, density: Density) -> Self {
        let mut t = match mode {
            ThemeMode::Dark => Self {
                mode,
                density,
                bg: HslaTuple::new(0.03, 0.04, 0.061, 1.0),
                surface: HslaTuple::new(0.08, 0.04, 0.106, 1.0),
                surface_raised: HslaTuple::new(0.08, 0.05, 0.13, 1.0),
                glass: HslaTuple::new(0.08, 0.04, 0.106, 1.0),
                glass_strong: HslaTuple::new(0.03, 0.04, 0.061, 1.0),
                glass_ultra: HslaTuple::new(0.08, 0.04, 0.13, 0.55),
                ink: HslaTuple::new(0.03, 0.04, 0.061, 1.0),
                text: HslaTuple::new(0.153, 0.08, 0.786, 1.0),
                text_muted: HslaTuple::new(0.11, 0.03, 0.516, 1.0),
                text_faint: HslaTuple::new(0.10, 0.03, 0.38, 1.0),
                accent: HslaTuple::new(0.067, 0.81, 0.606, 1.0),
                accent_muted: HslaTuple::new(0.056, 0.36, 0.13, 1.0),
                good: HslaTuple::new(0.20, 0.35, 0.51, 1.0),
                warn: HslaTuple::new(0.065, 0.71, 0.51, 1.0),
                danger: HslaTuple::new(0.014, 0.60, 0.54, 1.0),
                hairline: HslaTuple::new(0.10, 0.03, 0.20, 1.0),
                hairline_bright: HslaTuple::new(0.08, 0.04, 0.25, 1.0),
                selection: HslaTuple::new(0.067, 0.50, 0.22, 1.0),
                focus_ring: HslaTuple::new(0.067, 0.81, 0.606, 1.0),
                space_1: 0.0,
                space_2: 0.0,
                space_3: 0.0,
                space_4: 0.0,
                space_5: 0.0,
                space_6: 0.0,
            },
            ThemeMode::Light => Self {
                mode,
                density,
                bg: HslaTuple::new(0.14, 0.40, 0.97, 1.0),
                surface: HslaTuple::new(0.14, 0.20, 0.99, 1.0),
                surface_raised: HslaTuple::new(0.0, 0.0, 1.0, 1.0),
                glass: HslaTuple::new(0.14, 0.20, 0.99, 1.0),
                glass_strong: HslaTuple::new(0.14, 0.40, 0.97, 1.0),
                glass_ultra: HslaTuple::new(0.067, 0.20, 0.94, 0.70),
                ink: HslaTuple::new(0.14, 0.40, 0.97, 1.0),
                text: HslaTuple::new(0.03, 0.04, 0.08, 1.0),
                text_muted: HslaTuple::new(0.08, 0.04, 0.32, 1.0),
                text_faint: HslaTuple::new(0.08, 0.03, 0.45, 1.0),
                accent: HslaTuple::new(0.067, 0.81, 0.48, 1.0),
                accent_muted: HslaTuple::new(0.067, 0.40, 0.90, 1.0),
                good: HslaTuple::new(0.20, 0.40, 0.38, 1.0),
                warn: HslaTuple::new(0.065, 0.71, 0.42, 1.0),
                danger: HslaTuple::new(0.014, 0.62, 0.42, 1.0),
                hairline: HslaTuple::new(0.10, 0.06, 0.82, 1.0),
                hairline_bright: HslaTuple::new(0.10, 0.08, 0.72, 1.0),
                selection: HslaTuple::new(0.067, 0.45, 0.88, 1.0),
                focus_ring: HslaTuple::new(0.067, 0.81, 0.48, 1.0),
                space_1: 0.0,
                space_2: 0.0,
                space_3: 0.0,
                space_4: 0.0,
                space_5: 0.0,
                space_6: 0.0,
            },
        };
        t.apply_density();
        t
    }
}
