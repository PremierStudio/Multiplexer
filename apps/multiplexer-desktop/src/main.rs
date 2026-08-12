//! Phase 0.4 GPUI shell.
//!
//! Testable chrome lives in `multiplexer-shell`. This binary only projects
//! that chrome into a window. Do not put assertions here: CI has no display.

use gpui::{
    div, prelude::*, px, relative, rgb, size, App, Application, Bounds, Context, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use multiplexer_layout::{Axis, LayoutNode, PaneId};
use multiplexer_shell::DesktopChrome;

struct ShellView {
    chrome: DesktopChrome,
}

impl Render for ShellView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.chrome.layout.focus();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1a1a1a))
            .text_color(rgb(0xe8e8e8))
            .child(
                div()
                    .h(px(36.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .bg(rgb(0x141414))
                    .border_b_1()
                    .border_color(rgb(0x2e2e2e))
                    .child(self.chrome.connection_label()),
            )
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .child(project_node(&self.chrome.layout.primary().root, focus)),
            )
    }
}

fn project_node(node: &LayoutNode, focus: PaneId) -> gpui::AnyElement {
    match node {
        LayoutNode::Split(split) => {
            let container = match split.axis {
                Axis::Horizontal => div().flex().flex_row(),
                Axis::Vertical => div().flex().flex_col(),
            };
            container
                .size_full()
                .child(sized_child(&split.first, split.ratio, split.axis, focus))
                .child(sized_child(
                    &split.second,
                    1.0 - split.ratio,
                    split.axis,
                    focus,
                ))
                .into_any()
        }
        LayoutNode::Leaf { pane, ghost, .. } => pane_frame(*pane, *ghost, focus).into_any(),
    }
}

fn sized_child(node: &LayoutNode, ratio: f32, axis: Axis, focus: PaneId) -> gpui::Div {
    let sized = match axis {
        Axis::Horizontal => div().w(relative(ratio)).h_full(),
        Axis::Vertical => div().h(relative(ratio)).w_full(),
    };
    sized.flex_grow().child(project_node(node, focus))
}

fn pane_frame(pane: PaneId, ghost: bool, focus: PaneId) -> gpui::Div {
    let focused = pane == focus;
    let bg = if ghost {
        rgb(0x2a2a2a)
    } else if focused {
        rgb(0x252830)
    } else {
        rgb(0x222222)
    };
    let label = if ghost {
        format!("ghost {}", pane.0)
    } else {
        format!("pane {}", pane.0)
    };
    div()
        .size_full()
        .m(px(1.0))
        .bg(bg)
        .border_1()
        .border_color(rgb(0x3a3a3a))
        .flex()
        .justify_center()
        .items_center()
        .child(label)
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let chrome = DesktopChrome::default_outlook();
        let title: SharedString = chrome.title.clone().into();
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(title),
                    appears_transparent: false,
                    traffic_light_position: None,
                }),
                ..Default::default()
            },
            |_, cx| cx.new(|_| ShellView { chrome }),
        )
        .expect("open Multiplexer window");
        // Windows stays blank until the app is activated and paints once.
        cx.activate(true);
    });
}
