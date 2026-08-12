//! Pure, serializable pane layout tree (plan/10).
//!
//! No GPUI types live here. The desktop shell projects this tree into
//! elements each frame.

mod tree;

pub use tree::{
    Axis, LayoutError, LayoutForest, LayoutNode, PaneId, SplitNode, WindowId, WindowRoot,
};
