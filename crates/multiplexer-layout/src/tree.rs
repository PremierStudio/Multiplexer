//! Binary split tree + forest of OS windows.

use serde::{Deserialize, Serialize};

/// Identifies a pane's content slot. Stable across detach/redock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

/// Identifies an OS window that owns a layout root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

/// Split direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Errors from layout mutations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("unknown window {0}")]
    UnknownWindow(u64),
    #[error("unknown pane {0}")]
    UnknownPane(u64),
    #[error("ratio {0} is not in (0, 1)")]
    InvalidRatio(String),
    #[error("cannot close the last pane in the primary window")]
    LastPane,
    #[error("pane {0} is not a ghost slot")]
    NotGhost(u64),
}

/// A node in a window's layout tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutNode {
    Split(SplitNode),
    Leaf {
        pane: PaneId,
        /// Optional stacked tabs. First is the visible tab when present.
        tabs: Vec<PaneId>,
        /// True when this leaf is a placeholder left by a detach.
        ghost: bool,
    },
}

/// Binary split.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitNode {
    pub axis: Axis,
    /// Fraction of space given to `first` (exclusive 0..1).
    pub ratio: f32,
    pub first: Box<LayoutNode>,
    pub second: Box<LayoutNode>,
}

/// One OS window and its root node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowRoot {
    pub id: WindowId,
    pub root: LayoutNode,
    pub primary: bool,
}

/// Forest of windows plus focus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutForest {
    windows: Vec<WindowRoot>,
    focus: PaneId,
    next_pane: u64,
    next_window: u64,
}

impl LayoutForest {
    /// Outlook-style default: left | center | right, horizontal splits.
    pub fn default_outlook() -> Self {
        let left = PaneId(1);
        let center = PaneId(2);
        let right = PaneId(3);
        let root = LayoutNode::Split(SplitNode {
            axis: Axis::Horizontal,
            ratio: 0.2,
            first: Box::new(leaf(left)),
            second: Box::new(LayoutNode::Split(SplitNode {
                axis: Axis::Horizontal,
                ratio: 0.75,
                first: Box::new(leaf(center)),
                second: Box::new(leaf(right)),
            })),
        });
        Self {
            windows: vec![WindowRoot {
                id: WindowId(1),
                root,
                primary: true,
            }],
            focus: center,
            next_pane: 4,
            next_window: 2,
        }
    }

    pub fn focus(&self) -> PaneId {
        self.focus
    }

    pub fn set_focus(&mut self, pane: PaneId) -> Result<(), LayoutError> {
        if !self.contains_pane(pane) {
            return Err(LayoutError::UnknownPane(pane.0));
        }
        self.focus = pane;
        Ok(())
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn primary(&self) -> &WindowRoot {
        self.windows
            .iter()
            .find(|w| w.primary)
            .expect("primary window")
    }

    pub fn windows(&self) -> &[WindowRoot] {
        &self.windows
    }

    pub fn contains_pane(&self, pane: PaneId) -> bool {
        self.windows.iter().any(|w| node_contains(&w.root, pane))
    }

    /// Split `target` along `axis`, putting a new pane on `second`.
    pub fn split(&mut self, target: PaneId, axis: Axis, ratio: f32) -> Result<PaneId, LayoutError> {
        validate_ratio(ratio)?;
        if !self.contains_pane(target) {
            return Err(LayoutError::UnknownPane(target.0));
        }
        let new_id = PaneId(self.next_pane);
        self.next_pane += 1;
        for w in &mut self.windows {
            if replace_pane(&mut w.root, target, |old| {
                LayoutNode::Split(SplitNode {
                    axis,
                    ratio,
                    first: Box::new(old),
                    second: Box::new(leaf(new_id)),
                })
            }) {
                return Ok(new_id);
            }
        }
        Err(LayoutError::UnknownPane(target.0))
    }

    /// Close a pane. Collapses single-child splits. Refuses closing the last
    /// live (non-ghost) pane in the primary window.
    pub fn close(&mut self, pane: PaneId) -> Result<(), LayoutError> {
        if !self.contains_pane(pane) {
            return Err(LayoutError::UnknownPane(pane.0));
        }
        let primary_id = self.primary().id;
        if self.live_pane_count_in(primary_id) == 1
            && window_contains(self.window(primary_id)?, pane)
            && !is_ghost_of(self.window(primary_id)?, pane)
        {
            return Err(LayoutError::LastPane);
        }
        let win_id = self
            .windows
            .iter()
            .find(|w| window_contains(w, pane) && !is_ghost_of(w, pane))
            .or_else(|| self.windows.iter().find(|w| window_contains(w, pane)))
            .map(|w| w.id)
            .ok_or(LayoutError::UnknownPane(pane.0))?;
        {
            let win = self.window_mut(win_id)?;
            win.root = remove_pane(win.root.clone(), pane);
        }
        if is_empty_popout(self.window(win_id)?) {
            self.windows.retain(|w| w.id != win_id);
        }
        if self.focus == pane {
            self.focus = first_live_pane(self).unwrap_or(pane);
        }
        Ok(())
    }

    /// Detach a live pane into a new window. Leaves a ghost in the old slot.
    pub fn detach(&mut self, pane: PaneId) -> Result<WindowId, LayoutError> {
        if !self.contains_pane(pane) {
            return Err(LayoutError::UnknownPane(pane.0));
        }
        if is_ghost_anywhere(self, pane) {
            return Err(LayoutError::UnknownPane(pane.0));
        }
        let new_win = WindowId(self.next_window);
        self.next_window += 1;
        for w in &mut self.windows {
            if replace_pane(&mut w.root, pane, |old| match old {
                LayoutNode::Leaf { pane: p, tabs, .. } => LayoutNode::Leaf {
                    pane: p,
                    tabs,
                    ghost: true,
                },
                other => other,
            }) {
                self.windows.push(WindowRoot {
                    id: new_win,
                    root: leaf(pane),
                    primary: false,
                });
                return Ok(new_win);
            }
        }
        Err(LayoutError::UnknownPane(pane.0))
    }

    /// Redock a detached window's pane into its ghost slot.
    pub fn redock(&mut self, pane: PaneId) -> Result<(), LayoutError> {
        let pop_idx = self
            .windows
            .iter()
            .position(|w| !w.primary && node_contains(&w.root, pane))
            .ok_or(LayoutError::UnknownPane(pane.0))?;
        let mut found_ghost = false;
        for w in &mut self.windows {
            let _ = replace_pane(&mut w.root, pane, |old| match old {
                LayoutNode::Leaf {
                    pane: p,
                    tabs,
                    ghost: true,
                } => {
                    found_ghost = true;
                    LayoutNode::Leaf {
                        pane: p,
                        tabs,
                        ghost: false,
                    }
                }
                other => other,
            });
        }
        if !found_ghost {
            return Err(LayoutError::NotGhost(pane.0));
        }
        self.windows.remove(pop_idx);
        Ok(())
    }

    pub fn set_ratio(&mut self, pane_in_split: PaneId, ratio: f32) -> Result<(), LayoutError> {
        validate_ratio(ratio)?;
        for w in &mut self.windows {
            if set_ratio_near(&mut w.root, pane_in_split, ratio) {
                return Ok(());
            }
        }
        Err(LayoutError::UnknownPane(pane_in_split.0))
    }

    fn window(&self, id: WindowId) -> Result<&WindowRoot, LayoutError> {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .ok_or(LayoutError::UnknownWindow(id.0))
    }

    fn window_mut(&mut self, id: WindowId) -> Result<&mut WindowRoot, LayoutError> {
        self.windows
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or(LayoutError::UnknownWindow(id.0))
    }

    fn live_pane_count_in(&self, id: WindowId) -> usize {
        self.windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| count_live(&w.root))
            .unwrap_or(0)
    }
}

fn leaf(pane: PaneId) -> LayoutNode {
    LayoutNode::Leaf {
        pane,
        tabs: Vec::new(),
        ghost: false,
    }
}

fn validate_ratio(ratio: f32) -> Result<(), LayoutError> {
    if ratio > 0.0 && ratio < 1.0 && ratio.is_finite() {
        Ok(())
    } else {
        Err(LayoutError::InvalidRatio(ratio.to_string()))
    }
}

fn node_contains(node: &LayoutNode, pane: PaneId) -> bool {
    match node {
        LayoutNode::Leaf { pane: p, tabs, .. } => *p == pane || tabs.contains(&pane),
        LayoutNode::Split(s) => node_contains(&s.first, pane) || node_contains(&s.second, pane),
    }
}

fn window_contains(win: &WindowRoot, pane: PaneId) -> bool {
    node_contains(&win.root, pane)
}

fn is_ghost_of(win: &WindowRoot, pane: PaneId) -> bool {
    fn walk(n: &LayoutNode, pane: PaneId) -> bool {
        match n {
            LayoutNode::Leaf { pane: p, ghost, .. } => *p == pane && *ghost,
            LayoutNode::Split(s) => walk(&s.first, pane) || walk(&s.second, pane),
        }
    }
    walk(&win.root, pane)
}

fn is_ghost_anywhere(forest: &LayoutForest, pane: PaneId) -> bool {
    forest.windows.iter().any(|w| is_ghost_of(w, pane))
}

fn is_empty_popout(win: &WindowRoot) -> bool {
    !win.primary && count_live(&win.root) == 0
}

fn count_live(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Leaf { ghost, .. } => usize::from(!*ghost),
        LayoutNode::Split(s) => count_live(&s.first) + count_live(&s.second),
    }
}

fn first_live_pane(forest: &LayoutForest) -> Option<PaneId> {
    fn walk(n: &LayoutNode) -> Option<PaneId> {
        match n {
            LayoutNode::Leaf { pane, ghost, .. } if !*ghost => Some(*pane),
            LayoutNode::Leaf { .. } => None,
            LayoutNode::Split(s) => walk(&s.first).or_else(|| walk(&s.second)),
        }
    }
    forest.windows.iter().find_map(|w| walk(&w.root))
}

fn replace_pane(
    node: &mut LayoutNode,
    target: PaneId,
    mut f: impl FnMut(LayoutNode) -> LayoutNode,
) -> bool {
    replace_pane_inner(node, target, &mut f)
}

fn replace_pane_inner(
    node: &mut LayoutNode,
    target: PaneId,
    f: &mut impl FnMut(LayoutNode) -> LayoutNode,
) -> bool {
    match node {
        LayoutNode::Leaf { pane, .. } if *pane == target => {
            let old = std::mem::replace(
                node,
                LayoutNode::Leaf {
                    pane: target,
                    tabs: Vec::new(),
                    ghost: false,
                },
            );
            *node = f(old);
            true
        }
        LayoutNode::Split(s) => {
            replace_pane_inner(&mut s.first, target, f)
                || replace_pane_inner(&mut s.second, target, f)
        }
        _ => false,
    }
}

fn remove_pane(node: LayoutNode, pane: PaneId) -> LayoutNode {
    match node {
        LayoutNode::Leaf { pane: p, .. } if p == pane => LayoutNode::Leaf {
            pane: p,
            tabs: Vec::new(),
            ghost: true,
        },
        LayoutNode::Split(s) => {
            let first = remove_pane(*s.first, pane);
            let second = remove_pane(*s.second, pane);
            match (&first, &second) {
                (
                    LayoutNode::Leaf {
                        pane: p,
                        ghost: true,
                        ..
                    },
                    _,
                ) if *p == pane => second,
                (
                    _,
                    LayoutNode::Leaf {
                        pane: p,
                        ghost: true,
                        ..
                    },
                ) if *p == pane => first,
                _ => LayoutNode::Split(SplitNode {
                    axis: s.axis,
                    ratio: s.ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
            }
        }
        other => other,
    }
}

fn set_ratio_near(node: &mut LayoutNode, pane: PaneId, ratio: f32) -> bool {
    match node {
        LayoutNode::Split(s) => {
            if node_contains(&s.first, pane) || node_contains(&s.second, pane) {
                if matches!(&*s.first, LayoutNode::Leaf { pane: p, .. } if *p == pane)
                    || matches!(&*s.second, LayoutNode::Leaf { pane: p, .. } if *p == pane)
                {
                    s.ratio = ratio;
                    return true;
                }
                return set_ratio_near(&mut s.first, pane, ratio)
                    || set_ratio_near(&mut s.second, pane, ratio);
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn default_has_three_live_panes_and_center_focus() {
        let f = LayoutForest::default_outlook();
        assert_eq!(f.window_count(), 1);
        assert_eq!(f.focus(), PaneId(2));
        assert!(f.contains_pane(PaneId(1)));
        assert!(f.contains_pane(PaneId(2)));
        assert!(f.contains_pane(PaneId(3)));
    }

    #[test]
    fn invalid_ratio_rejected() {
        let mut f = LayoutForest::default_outlook();
        assert!(matches!(
            f.split(PaneId(2), Axis::Vertical, 0.0),
            Err(LayoutError::InvalidRatio(_))
        ));
        assert!(matches!(
            f.split(PaneId(2), Axis::Vertical, 1.0),
            Err(LayoutError::InvalidRatio(_))
        ));
        assert!(matches!(
            f.split(PaneId(2), Axis::Vertical, -0.1),
            Err(LayoutError::InvalidRatio(_))
        ));
        assert!(matches!(
            f.split(PaneId(2), Axis::Vertical, f32::NAN),
            Err(LayoutError::InvalidRatio(_))
        ));
        assert!(matches!(
            f.split(PaneId(2), Axis::Vertical, f32::INFINITY),
            Err(LayoutError::InvalidRatio(_))
        ));
    }

    #[test]
    fn close_last_primary_pane_fails() {
        let mut f = LayoutForest::default_outlook();
        f.close(PaneId(1)).unwrap();
        f.close(PaneId(3)).unwrap();
        assert_eq!(f.close(PaneId(2)), Err(LayoutError::LastPane));
    }
}
