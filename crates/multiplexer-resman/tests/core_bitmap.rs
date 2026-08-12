use multiplexer_resman::{CoreBitmap, ResmanError, SessionAlloc, SessionId};
use proptest::collection::vec;
use proptest::prelude::*;

fn alloc_cores(bitmap: &mut CoreBitmap, session: SessionId, count: usize) -> SessionAlloc {
    bitmap
        .allocate(session, count, Some(1024))
        .expect("allocation should succeed")
}

#[test]
fn new_zero_cores_errors() {
    assert_eq!(
        CoreBitmap::new(0).err(),
        Some(ResmanError::InvalidCoreCount)
    );
}

#[test]
fn new_reserves_core_zero_and_one() {
    let mut bitmap = CoreBitmap::new(8).expect("valid core count");
    // Cores 0 and 1 are reserved when n_cores > 2.
    assert_eq!(bitmap.free_enabled_count(), 6);
    assert_eq!(bitmap.enabled_non_reserved_count(), 6);

    let alloc = alloc_cores(&mut bitmap, SessionId(1), 3);
    assert_eq!(alloc.session, SessionId(1));
    assert_eq!(alloc.cores, vec![2, 3, 4]);
    assert_eq!(alloc.memory_cap_bytes, Some(1024));
}

#[test]
fn disabled_core_is_skipped() {
    let mut bitmap = CoreBitmap::new(6).expect("valid core count");
    bitmap.set_enabled(3, false).expect("core in range");

    let alloc = alloc_cores(&mut bitmap, SessionId(1), 2);
    assert_eq!(alloc.cores, vec![2, 4]);
}

#[test]
fn reserve_then_allocate_skips_reserved() {
    let mut bitmap = CoreBitmap::new(6).expect("valid core count");
    bitmap
        .reserve(&[3])
        .expect("core in range and not reserved");

    let alloc = alloc_cores(&mut bitmap, SessionId(1), 2);
    assert_eq!(alloc.cores, vec![2, 4]);
}

#[test]
fn allocate_fails_when_not_enough_cores() {
    let mut bitmap = CoreBitmap::new(8).expect("valid core count");
    alloc_cores(&mut bitmap, SessionId(1), 6);
    // All 6 non-reserved cores are taken; one more must fail.
    assert_eq!(
        bitmap.allocate(SessionId(2), 1, None).err(),
        Some(ResmanError::InsufficientCores {
            needed: 1,
            available: 0
        })
    );
}

#[test]
fn double_allocate_same_session_errors() {
    let mut bitmap = CoreBitmap::new(8).expect("valid core count");
    alloc_cores(&mut bitmap, SessionId(1), 2);
    assert_eq!(
        bitmap.allocate(SessionId(1), 1, None).err(),
        Some(ResmanError::SessionAlreadyAllocated(1))
    );
}

#[test]
fn free_unknown_session_errors() {
    let mut bitmap = CoreBitmap::new(8).expect("valid core count");
    assert_eq!(
        bitmap.free(SessionId(99)).err(),
        Some(ResmanError::UnknownSession(99))
    );
}

#[test]
fn free_then_reallocate_works() {
    let mut bitmap = CoreBitmap::new(8).expect("valid core count");
    let first = alloc_cores(&mut bitmap, SessionId(1), 3);
    assert_eq!(first.cores, vec![2, 3, 4]);

    let freed = bitmap.free(SessionId(1)).expect("session known");
    assert_eq!(freed.cores, first.cores);

    assert_eq!(bitmap.allocated(SessionId(1)), None);

    let second = alloc_cores(&mut bitmap, SessionId(1), 3);
    assert_eq!(second.cores, vec![2, 3, 4]);
}

#[test]
fn reserve_is_idempotent_and_deduplicates() {
    let mut bitmap = CoreBitmap::new(8).expect("valid core count");
    bitmap.reserve(&[5, 5, 6]).expect("valid cores");
    assert_eq!(bitmap.enabled_non_reserved_count(), 4); // 2,3,4,7
}

#[test]
fn set_enabled_out_of_range_errors() {
    let mut bitmap = CoreBitmap::new(4).expect("valid core count");
    assert_eq!(
        bitmap.set_enabled(9, true).err(),
        Some(ResmanError::CoreOutOfRange(9))
    );
}

#[test]
fn allocate_out_of_range_and_reserved_core_errors() {
    let mut bitmap = CoreBitmap::new(4).expect("valid core count");
    assert_eq!(
        bitmap.reserve(&[7]).err(),
        Some(ResmanError::CoreOutOfRange(7))
    );
    assert_eq!(
        bitmap.reserve(&[1]).err(),
        Some(ResmanError::CoreReserved(1))
    );
}

#[test]
fn allocate_zero_cores_yields_empty_alloc() {
    let mut bitmap = CoreBitmap::new(4).expect("valid core count");
    let alloc = bitmap
        .allocate(SessionId(1), 0, None)
        .expect("zero-count allocation is a no-op");
    assert!(alloc.cores.is_empty());
    assert_eq!(bitmap.free_enabled_count(), 2);
}

#[test]
fn small_bitmap_reserves_nothing() {
    // n_cores <= 2: no cores reserved.
    let mut bitmap = CoreBitmap::new(2).expect("valid core count");
    assert_eq!(bitmap.enabled_non_reserved_count(), 2);
    let alloc = alloc_cores(&mut bitmap, SessionId(1), 2);
    assert_eq!(alloc.cores, vec![0, 1]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn invariant_total_cores_consistent(
        n in 4usize..32,
        allocations in vec(1usize..8, 0..8),
    ) {
        let mut bitmap = CoreBitmap::new(n).expect("valid core count");
        let mut session_total: usize = 0;
        for (i, count) in allocations.iter().enumerate() {
            if bitmap.free_enabled_count() >= *count {
                let alloc = alloc_cores(&mut bitmap, SessionId(i as u64), *count);
                session_total += alloc.cores.len();
            }
        }
        prop_assert_eq!(
            session_total + bitmap.free_enabled_count(),
            bitmap.enabled_non_reserved_count(),
            "sum of session cores plus free cores must equal enabled non-reserved cores"
        );
    }

    #[test]
    fn reserved_cores_never_allocated(
        n in 4usize..32,
        extra_reserved in vec(1usize..31, 0..4),
        allocations in vec(1usize..8, 0..8),
    ) {
        let mut bitmap = CoreBitmap::new(n).expect("valid core count");
        // 0 and 1 are always reserved for n > 2; add random extras.
        let mut reserved: Vec<usize> = vec![0, 1];
        for core in extra_reserved {
            if bitmap.reserve(&[core]).is_ok() {
                reserved.push(core);
            }
        }
        for (i, count) in allocations.iter().enumerate() {
            if bitmap.free_enabled_count() >= *count {
                let alloc = alloc_cores(&mut bitmap, SessionId(i as u64), *count);
                for core in &alloc.cores {
                    prop_assert!(
                        !reserved.contains(core),
                        "reserved core {} must never be allocated",
                        core
                    );
                }
            }
        }
    }
}
