//! Shared GPUI primitives. No raw `hsla(` in `main.rs`.

use gpui::{div, prelude::*, px, svg, Context, MouseButton, SharedString};

use crate::theme::Theme;
use crate::ShellView;
use multiplexer_shell::{ChromeGlyph, EmptyStateSpec};

pub fn chrome_icon(glyph: ChromeGlyph, size: f32) -> gpui::AnyElement {
    svg()
        .path(glyph.icon_file())
        .size(px(size))
        .flex_shrink_0()
        .into_any()
}

pub fn path_icon(path: &str, size: f32) -> gpui::AnyElement {
    if path.ends_with(".svg") {
        svg()
            .path(SharedString::from(path.to_owned()))
            .size(px(size))
            .flex_shrink_0()
            .into_any()
    } else {
        div().text_size(px(size)).child(path.to_owned()).into_any()
    }
}

pub fn glass_pane() -> gpui::Div {
    div().bg(Theme::ink()).overflow_hidden().min_h_0()
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
        .gap_2()
        .px_8()
        .pt_10()
        .text_color(Theme::muted())
        .child(
            div()
                .text_size(Theme::text_title())
                .text_color(Theme::text())
                .child(spec.title),
        )
        .child(
            div()
                .text_size(Theme::text_caption())
                .text_color(Theme::faint())
                .child(spec.body),
        )
}

pub fn chip(
    label: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(label))
        .h(px(28.0))
        .px_3()
        .rounded_lg()
        .flex()
        .items_center()
        .bg(Theme::raised())
        .text_color(Theme::muted())
        .text_size(Theme::text_caption())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::selection()).text_color(Theme::text()))
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
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .bg(Theme::raised())
        .text_color(if label == "Stop" {
            Theme::danger()
        } else {
            Theme::text()
        })
        .text_size(Theme::text_caption())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::selection()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .overflow_hidden()
        .child(label)
        .into_any()
}

pub fn primary_btn(
    label: &'static str,
    hint: &'static str,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(SharedString::from(format!("{label}-{hint}")))
        .h(px(28.0))
        .px_3()
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .bg(Theme::selection())
        .text_color(Theme::text())
        .text_size(Theme::text_caption())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::raised()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(label)
        .into_any()
}

pub fn icon_btn(
    glyph: ChromeGlyph,
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
        .bg(Theme::transparent())
        .text_color(Theme::muted())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::selection()).text_color(Theme::text()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(chrome_icon(glyph, 16.0))
        .into_any()
}

pub fn rail_icon(
    glyph: ChromeGlyph,
    hint: impl Into<String>,
    on: bool,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    let hint = hint.into();
    div()
        .id(SharedString::from(hint))
        .w(px(32.0))
        .h(px(32.0))
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .bg(if on {
            Theme::selection()
        } else {
            Theme::transparent()
        })
        .text_color(if on { Theme::text() } else { Theme::muted() })
        .cursor_pointer()
        .hover(|s| s.bg(Theme::selection()).text_color(Theme::text()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(chrome_icon(glyph, 16.0))
        .into_any()
}

pub fn pill(text: impl Into<String>, mark: &'static str) -> impl IntoElement {
    let mark = mark.trim();
    div()
        .h(px(22.0))
        .px_2()
        .rounded_lg()
        .flex()
        .items_center()
        .gap_1()
        .bg(Theme::raised())
        .text_size(Theme::text_caption())
        .text_color(Theme::muted())
        .child(if mark.is_empty() {
            SharedString::from("")
        } else {
            SharedString::from(mark)
        })
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
        .hover(|s| s.bg(Theme::selection()))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .child(pill(text, mark))
        .into_any()
}
