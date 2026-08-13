//! Shared GPUI primitives. No raw `hsla(` in `main.rs`.

use gpui::{div, prelude::*, px, Context, MouseButton, SharedString};

use crate::theme::Theme;
use crate::ShellView;
use multiplexer_shell::{empty_state_tiles, EmptyStateSpec};

pub fn glass_pane() -> gpui::Div {
    div()
        .bg(Theme::glass())
        .border_color(Theme::hairline())
        .overflow_hidden()
        .min_h_0()
}

pub fn glass_bar() -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .bg(Theme::ink())
        .border_color(Theme::hairline())
}

pub fn empty_center() -> gpui::Div {
    let spec = EmptyStateSpec::chat();
    div()
        .flex_1()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .px_8()
        .text_color(Theme::muted())
        .child(
            div()
                .text_size(Theme::text_body())
                .text_color(Theme::text())
                .child(spec.title),
        )
        .child(
            div()
                .text_size(Theme::text_caption())
                .text_color(Theme::faint())
                .child(spec.body),
        )
        .child(
            div()
                .text_size(Theme::text_caption())
                .text_color(Theme::faint())
                .child(empty_state_tiles().join("   ·   ")),
        )
}

pub fn chip(
    label: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(label))
        .px_2()
        .py_1()
        .rounded_lg()
        .bg(Theme::wash())
        .border_1()
        .border_color(Theme::hairline())
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(label)
        .into_any()
}

pub fn ghost_btn(
    label: &'static str,
    hint: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(format!("{label}-{hint}")))
        .h(px(28.0))
        .px_3()
        .flex()
        .items_center()
        .gap_2()
        .rounded_lg()
        .border_1()
        .border_color(Theme::hairline())
        .bg(if label == "Stop" {
            Theme::danger()
        } else if label == "Send" || label == "commit & sync" {
            Theme::accent()
        } else {
            Theme::transparent()
        })
        .text_color(if label == "Send" || label == "commit & sync" {
            Theme::ink()
        } else {
            Theme::text()
        })
        .cursor_pointer()
        .hover(|s| s.bg(Theme::hover_strong()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .overflow_hidden()
        .child(label)
        .into_any()
}

pub fn icon_btn(
    mark: &'static str,
    hint: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(hint.to_owned()))
        .w(Theme::icon_size())
        .h(Theme::icon_size())
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .bg(Theme::ghost_fill())
        .text_color(Theme::muted())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::selection()).text_color(Theme::text()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(mark)
        .into_any()
}

pub fn pill(text: impl Into<String>, mark: &'static str) -> impl IntoElement {
    div()
        .h(px(20.0))
        .px_2()
        .rounded_lg()
        .flex()
        .items_center()
        .gap_1()
        .bg(Theme::glass_ultra())
        .border_1()
        .border_color(Theme::hairline())
        .text_color(Theme::muted())
        .child(mark)
        .child(text.into())
}

pub fn click_pill(
    id: &'static str,
    text: impl Into<String>,
    mark: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(pill(text, mark))
        .into_any()
}
