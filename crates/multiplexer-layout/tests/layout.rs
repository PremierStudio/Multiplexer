use multiplexer_layout::{
    Axis, LayoutError, LayoutForest, LayoutNode, PaneId, SplitNode, WindowId,
};

fn live_ids(node: &LayoutNode) -> Vec<PaneId> {
    match node {
        LayoutNode::Leaf { pane, ghost, .. } if !*ghost => vec![*pane],
        LayoutNode::Leaf { .. } => vec![],
        LayoutNode::Split(s) => {
            let mut ids = live_ids(&s.first);
            ids.extend(live_ids(&s.second));
            ids
        }
    }
}

fn split_holding(node: &LayoutNode, pane: PaneId) -> Option<&SplitNode> {
    match node {
        LayoutNode::Split(s) => {
            let here = matches!(&*s.first, LayoutNode::Leaf { pane: p, .. } if *p == pane)
                || matches!(&*s.second, LayoutNode::Leaf { pane: p, .. } if *p == pane);
            if here {
                Some(s)
            } else {
                split_holding(&s.first, pane).or_else(|| split_holding(&s.second, pane))
            }
        }
        _ => None,
    }
}

fn is_live(node: &LayoutNode, pane: PaneId) -> bool {
    match node {
        LayoutNode::Leaf { pane: p, ghost, .. } if *p == pane => !*ghost,
        LayoutNode::Split(s) => is_live(&s.first, pane) || is_live(&s.second, pane),
        _ => false,
    }
}

fn leaf_tabs(node: &LayoutNode, pane: PaneId) -> Option<&[PaneId]> {
    match node {
        LayoutNode::Leaf { pane: p, tabs, .. } if *p == pane => Some(tabs),
        LayoutNode::Split(s) => leaf_tabs(&s.first, pane).or_else(|| leaf_tabs(&s.second, pane)),
        _ => None,
    }
}

#[test]
fn split_adds_pane_and_keeps_old() {
    let mut f = LayoutForest::default_outlook();
    let new = f.split(PaneId(2), Axis::Vertical, 0.5).unwrap();
    assert!(f.contains_pane(PaneId(2)));
    assert!(f.contains_pane(new));
    let split = split_holding(&f.primary().root, PaneId(2)).expect("pane 2 still in a split");
    assert_eq!(split.axis, Axis::Vertical);
    assert_eq!(split.ratio, 0.5);
    assert!(live_ids(&split.second).contains(&new));
}

#[test]
fn successive_splits_allocate_distinct_increasing_ids() {
    let mut f = LayoutForest::default_outlook();
    let a = f.split(PaneId(2), Axis::Vertical, 0.5).unwrap();
    let b = f.split(a, Axis::Horizontal, 0.4).unwrap();
    assert_eq!(a, PaneId(4));
    assert_eq!(b, PaneId(5));
    assert!(f.contains_pane(PaneId(1)));
    assert!(f.contains_pane(PaneId(2)));
    assert!(f.contains_pane(PaneId(3)));
    assert!(f.contains_pane(a));
    assert!(f.contains_pane(b));
}

#[test]
fn unknown_pane_errors() {
    let mut f = LayoutForest::default_outlook();
    assert_eq!(
        f.split(PaneId(99), Axis::Horizontal, 0.5),
        Err(LayoutError::UnknownPane(99))
    );
    assert_eq!(f.close(PaneId(99)), Err(LayoutError::UnknownPane(99)));
    assert_eq!(f.set_focus(PaneId(99)), Err(LayoutError::UnknownPane(99)));
}

#[test]
fn detach_creates_window_and_redock_restores() {
    let mut f = LayoutForest::default_outlook();
    let win = f.detach(PaneId(3)).unwrap();
    assert_eq!(win, WindowId(2));
    assert_eq!(f.window_count(), 2);
    assert!(f.windows().iter().any(|w| w.id == win && !w.primary));
    assert!(f.primary().primary);
    assert_eq!(f.primary().id, WindowId(1));
    f.redock(PaneId(3)).unwrap();
    assert_eq!(f.window_count(), 1);
    assert!(f.windows()[0].primary);
    assert_eq!(f.windows()[0].id, WindowId(1));
    assert!(f.contains_pane(PaneId(3)));
}

#[test]
fn successive_detaches_allocate_distinct_window_ids() {
    let mut f = LayoutForest::default_outlook();
    let first = f.detach(PaneId(3)).unwrap();
    let second = f.detach(PaneId(1)).unwrap();
    assert_eq!(first, WindowId(2));
    assert_eq!(second, WindowId(3));
    assert_ne!(first, second);
    assert_eq!(f.window_count(), 3);
    assert_eq!(
        f.windows().iter().filter(|w| w.id == WindowId(1)).count(),
        1
    );
    assert!(f.primary().primary);
    assert_eq!(f.primary().id, WindowId(1));
}

#[test]
fn detach_of_ghost_fails_and_second_live_detach_works() {
    let mut f = LayoutForest::default_outlook();
    f.detach(PaneId(3)).unwrap();
    assert_eq!(f.detach(PaneId(3)), Err(LayoutError::UnknownPane(3)));
    let win = f.detach(PaneId(2)).unwrap();
    assert_eq!(win, WindowId(3));
    assert_eq!(f.window_count(), 3);
}

#[test]
fn redock_without_ghost_fails() {
    let mut f = LayoutForest::default_outlook();
    assert_eq!(f.redock(PaneId(3)), Err(LayoutError::UnknownPane(3)));
}

#[test]
fn set_ratio_and_json_round_trip() {
    let mut f = LayoutForest::default_outlook();
    f.set_ratio(PaneId(1), 0.3).unwrap();
    let json = serde_json::to_string(&f).unwrap();
    let back: LayoutForest = serde_json::from_str(&json).unwrap();
    assert_eq!(f, back);
    let root = match &f.primary().root {
        LayoutNode::Split(s) => s,
        other => panic!("expected root split, got {other:?}"),
    };
    assert_eq!(root.ratio, 0.3);
    f.set_ratio(PaneId(2), 0.4).unwrap();
    let inner = split_holding(&f.primary().root, PaneId(2)).expect("inner split");
    assert_eq!(inner.ratio, 0.4);
    let root = match &f.primary().root {
        LayoutNode::Split(s) => s,
        other => panic!("expected root split, got {other:?}"),
    };
    assert_eq!(root.ratio, 0.3);
}

#[test]
fn set_ratio_rejects_invalid_and_unknown() {
    let mut f = LayoutForest::default_outlook();
    assert!(matches!(
        f.set_ratio(PaneId(2), 0.0),
        Err(LayoutError::InvalidRatio(_))
    ));
    assert!(matches!(
        f.set_ratio(PaneId(2), 1.0),
        Err(LayoutError::InvalidRatio(_))
    ));
    assert_eq!(
        f.set_ratio(PaneId(99), 0.5),
        Err(LayoutError::UnknownPane(99))
    );
}

#[test]
fn closing_moves_focus_to_a_live_pane() {
    let mut f = LayoutForest::default_outlook();
    f.set_focus(PaneId(1)).unwrap();
    f.close(PaneId(1)).unwrap();
    assert_ne!(f.focus(), PaneId(1));
    assert!(!f.contains_pane(PaneId(1)));
    assert!(f.contains_pane(PaneId(2)));
    assert!(f.contains_pane(PaneId(3)));
    assert!(f.contains_pane(f.focus()));
}

#[test]
fn close_removes_only_the_target_and_keeps_unfocused() {
    let mut f = LayoutForest::default_outlook();
    f.set_focus(PaneId(2)).unwrap();
    f.close(PaneId(1)).unwrap();
    assert_eq!(f.focus(), PaneId(2));
    assert!(!f.contains_pane(PaneId(1)));
    assert!(f.contains_pane(PaneId(2)));
    assert!(f.contains_pane(PaneId(3)));
    f.close(PaneId(3)).unwrap();
    assert!(!f.contains_pane(PaneId(3)));
    assert!(f.contains_pane(PaneId(2)));
    assert_eq!(f.focus(), PaneId(2));
}

#[test]
fn close_popout_removes_that_window_only() {
    let mut f = LayoutForest::default_outlook();
    f.detach(PaneId(3)).unwrap();
    assert_eq!(f.window_count(), 2);
    f.close(PaneId(1)).unwrap();
    assert_eq!(f.window_count(), 2);
    assert!(f.primary().primary);
    f.close(PaneId(3)).unwrap();
    assert_eq!(f.window_count(), 1);
    assert!(f.windows()[0].primary);
    assert_eq!(f.windows()[0].id, WindowId(1));
    assert!(f.contains_pane(PaneId(3)));
    f.redock(PaneId(3)).unwrap_err();
    f.close(PaneId(3)).unwrap();
    assert!(!f.contains_pane(PaneId(3)));
}

#[test]
fn close_unrelated_pane_keeps_detach_ghost() {
    let mut f = LayoutForest::default_outlook();
    f.detach(PaneId(3)).unwrap();
    f.close(PaneId(1)).unwrap();
    assert!(f.contains_pane(PaneId(3)));
    f.redock(PaneId(3)).unwrap();
    assert_eq!(f.window_count(), 1);
    assert!(f.contains_pane(PaneId(3)));
    assert!(f.windows()[0].primary);
}

#[test]
fn close_keeps_leading_and_inner_detach_ghosts() {
    let mut leading = LayoutForest::default_outlook();
    leading.detach(PaneId(1)).unwrap();
    leading.close(PaneId(3)).unwrap();
    assert!(leading.contains_pane(PaneId(1)));
    leading.redock(PaneId(1)).unwrap();
    assert_eq!(leading.window_count(), 1);
    assert!(is_live(&leading.primary().root, PaneId(1)));

    let mut inner = LayoutForest::default_outlook();
    inner.detach(PaneId(3)).unwrap();
    inner.close(PaneId(2)).unwrap();
    assert!(inner.contains_pane(PaneId(3)));
    inner.redock(PaneId(3)).unwrap();
    assert_eq!(inner.window_count(), 1);
    assert!(is_live(&inner.primary().root, PaneId(3)));
}

#[test]
fn closing_focused_pane_skips_leading_ghost() {
    let mut f = LayoutForest::default_outlook();
    f.detach(PaneId(1)).unwrap();
    f.set_focus(PaneId(2)).unwrap();
    f.close(PaneId(2)).unwrap();
    assert_eq!(f.focus(), PaneId(3));
    assert!(is_live(&f.primary().root, PaneId(3)));
}

#[test]
fn close_one_pane_of_split_popout_keeps_window() {
    let mut f = forest_from_json(serde_json::json!({
        "windows": [
            {
                "id": 1,
                "primary": true,
                "root": {
                    "Split": {
                        "axis": "Horizontal",
                        "ratio": 0.5,
                        "first": leaf_json(1, false),
                        "second": leaf_json(3, true)
                    }
                }
            },
            {
                "id": 2,
                "primary": false,
                "root": {
                    "Split": {
                        "axis": "Vertical",
                        "ratio": 0.5,
                        "first": leaf_json(3, false),
                        "second": leaf_json(4, false)
                    }
                }
            }
        ],
        "focus": 1,
        "next_pane": 5,
        "next_window": 3
    }));
    f.close(PaneId(4)).unwrap();
    assert_eq!(f.window_count(), 2);
    assert!(f.contains_pane(PaneId(3)));
    assert!(!f.contains_pane(PaneId(4)));
    assert!(f
        .windows()
        .iter()
        .any(|w| w.id == WindowId(2) && !w.primary));
}

fn forest_from_json(value: serde_json::Value) -> LayoutForest {
    serde_json::from_value(value).expect("layout fixture")
}

fn leaf_json(pane: u64, ghost: bool) -> serde_json::Value {
    serde_json::json!({ "Leaf": { "pane": pane, "tabs": [], "ghost": ghost } })
}

#[test]
fn last_pane_does_not_block_closing_popout_only_pane() {
    let mut f = forest_from_json(serde_json::json!({
        "windows": [
            {
                "id": 1,
                "primary": true,
                "root": {
                    "Split": {
                        "axis": "Horizontal",
                        "ratio": 0.5,
                        "first": leaf_json(2, false),
                        "second": leaf_json(3, true)
                    }
                }
            },
            {
                "id": 2,
                "primary": false,
                "root": {
                    "Split": {
                        "axis": "Vertical",
                        "ratio": 0.5,
                        "first": leaf_json(10, false),
                        "second": leaf_json(11, false)
                    }
                }
            }
        ],
        "focus": 2,
        "next_pane": 12,
        "next_window": 3
    }));
    assert_eq!(f.close(PaneId(11)), Ok(()));
    assert_eq!(f.window_count(), 2);
    assert!(f.contains_pane(PaneId(10)));
    assert!(!f.contains_pane(PaneId(11)));
    assert_eq!(f.close(PaneId(2)), Err(LayoutError::LastPane));
}

#[test]
fn tab_ids_are_contained_and_focusable() {
    let mut value = serde_json::to_value(LayoutForest::default_outlook()).unwrap();
    let windows = value.get_mut("windows").unwrap().as_array_mut().unwrap();
    fn add_tab(node: &mut serde_json::Value, pane: u64, tab: u64) -> bool {
        if let Some(leaf) = node.get_mut("Leaf") {
            if leaf.get("pane").and_then(|p| p.as_u64()) == Some(pane) {
                leaf["tabs"] = serde_json::json!([tab]);
                return true;
            }
            return false;
        }
        if let Some(split) = node.get_mut("Split") {
            return add_tab(&mut split["first"], pane, tab)
                || add_tab(&mut split["second"], pane, tab);
        }
        false
    }
    assert!(add_tab(&mut windows[0]["root"], 3, 99));
    let mut f: LayoutForest = serde_json::from_value(value).unwrap();
    assert!(f.contains_pane(PaneId(3)));
    assert!(f.contains_pane(PaneId(99)));
    f.set_focus(PaneId(99)).unwrap();
    assert_eq!(f.focus(), PaneId(99));
    assert_eq!(
        leaf_tabs(&f.primary().root, PaneId(3)),
        Some(&[PaneId(99)][..])
    );
}

proptest::proptest! {
    #[test]
    fn split_then_close_new_restores_three_live_in_primary(
        ratio in 0.05f32..0.95
    ) {
        let mut f = LayoutForest::default_outlook();
        let new = f.split(PaneId(2), Axis::Vertical, ratio).unwrap();
        f.close(new).unwrap();
        assert_eq!(f.window_count(), 1);
        assert!(f.contains_pane(PaneId(1)));
        assert!(f.contains_pane(PaneId(2)));
        assert!(f.contains_pane(PaneId(3)));
    }

    #[test]
    fn detach_redock_round_trip_preserves_primary_shape(
        which in 0u8..3
    ) {
        let pane = PaneId(u64::from(which) + 1);
        let mut f = LayoutForest::default_outlook();
        f.detach(pane).unwrap();
        f.redock(pane).unwrap();
        assert_eq!(f.window_count(), 1);
        assert!(f.contains_pane(PaneId(1)));
        assert!(f.contains_pane(PaneId(2)));
        assert!(f.contains_pane(PaneId(3)));
    }
}
