use multiplexer_resman::{format_core_bar, sample_cores, sample_cores_from};
use proptest::collection::vec;
use proptest::prelude::*;

#[test]
fn empty_usages_yields_empty_vec() {
    assert!(sample_cores_from(&[], &[]).is_empty());
    assert!(sample_cores_from(&[], &[0, 1]).is_empty());
}

#[test]
fn usage_is_copied_onto_each_sample() {
    let samples = sample_cores_from(&[12.5, 0.0, 99.25], &[]);
    assert_eq!(samples.len(), 3);
    assert_eq!(samples[0].index, 0);
    assert_eq!(samples[0].usage, 12.5);
    assert_eq!(samples[1].index, 1);
    assert_eq!(samples[1].usage, 0.0);
    assert_eq!(samples[2].index, 2);
    assert_eq!(samples[2].usage, 99.25);
}

#[test]
fn reserved_cores_are_marked() {
    let samples = sample_cores_from(&[1.0, 2.0, 3.0, 4.0], &[0, 2, 2, 99]);
    assert_eq!(
        samples.iter().map(|s| s.reserved).collect::<Vec<_>>(),
        vec![true, false, true, false]
    );
}

#[test]
fn unreserved_when_reserved_list_is_empty() {
    let samples = sample_cores_from(&[8.0, 9.0], &[]);
    assert!(samples.iter().all(|s| !s.reserved));
}

#[test]
fn format_core_bar_ten_ticks_and_percent() {
    assert_eq!(format_core_bar(0.0), "░░░░░░░░░░ 0%");
    assert_eq!(format_core_bar(41.0), "████░░░░░░ 41%");
    assert_eq!(format_core_bar(100.0), "██████████ 100%");
}

#[test]
fn format_core_bar_rounds_ticks_and_percent() {
    // 4% -> 0.4 ticks (round to 0); 5% -> 0.5 ticks (round to 1).
    assert_eq!(format_core_bar(4.0), "░░░░░░░░░░ 4%");
    assert_eq!(format_core_bar(5.0), "█░░░░░░░░░ 5%");
    // 1% would fill a tick if ceil were used.
    assert_eq!(format_core_bar(1.0), "░░░░░░░░░░ 1%");
    // 94% -> 9.4 ticks (round to 9); 95% -> 9.5 ticks (round to 10).
    assert_eq!(format_core_bar(94.0), "█████████░ 94%");
    assert_eq!(format_core_bar(95.0), "██████████ 95%");
    assert_eq!(format_core_bar(41.4), "████░░░░░░ 41%");
    assert_eq!(format_core_bar(41.6), "████░░░░░░ 42%");
}

#[test]
fn format_core_bar_clamps_and_treats_non_finite_as_zero() {
    assert_eq!(format_core_bar(-10.0), "░░░░░░░░░░ 0%");
    assert_eq!(format_core_bar(150.0), "██████████ 100%");
    assert_eq!(format_core_bar(f32::NAN), "░░░░░░░░░░ 0%");
    assert_eq!(format_core_bar(f32::INFINITY), "██████████ 100%");
    assert_eq!(format_core_bar(f32::NEG_INFINITY), "░░░░░░░░░░ 0%");
}

#[test]
fn sample_cores_marks_reserved_and_indexes_in_order() {
    let samples = sample_cores(&[0]);
    assert!(
        !samples.is_empty(),
        "sysinfo should report at least one logical CPU"
    );
    for (i, sample) in samples.iter().enumerate() {
        assert_eq!(sample.index, i);
        assert_eq!(sample.reserved, i == 0);
        assert!(sample.usage.is_finite());
        assert!(sample.usage >= 0.0);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn sample_len_matches_usages(
        usages in vec(-5.0f32..110.0, 0..32),
        reserved in vec(0usize..64, 0..8),
    ) {
        let samples = sample_cores_from(&usages, &reserved);
        prop_assert_eq!(samples.len(), usages.len());
        for (i, sample) in samples.iter().enumerate() {
            prop_assert_eq!(sample.index, i);
            prop_assert_eq!(sample.usage, usages[i]);
            prop_assert_eq!(sample.reserved, reserved.contains(&i));
        }
    }
}
