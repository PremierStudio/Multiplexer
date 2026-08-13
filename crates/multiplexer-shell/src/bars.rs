//! Text usage bars for inspector chrome.

/// Build a `width`-tick usage bar. `usage` is a percent, clamped to `0..=100`.
pub fn usage_bar(usage: f32, width: usize) -> String {
    // NaN.clamp stays NaN; the integer cast treats NaN as 0 filled ticks.
    let usage = usage.clamp(0.0, 100.0);
    let width = width.max(1);
    let filled = ((usage / 100.0) * width as f32).round() as usize;
    let filled = filled.min(width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::usage_bar;

    #[test]
    fn empty_at_zero() {
        assert_eq!(usage_bar(0.0, 10), "░░░░░░░░░░");
    }

    #[test]
    fn full_at_one_hundred() {
        assert_eq!(usage_bar(100.0, 10), "██████████");
    }

    #[test]
    fn half_at_fifty() {
        assert_eq!(usage_bar(50.0, 10), "█████░░░░░");
    }

    #[test]
    fn nan_is_empty() {
        assert_eq!(usage_bar(f32::NAN, 10), "░░░░░░░░░░");
    }
}
