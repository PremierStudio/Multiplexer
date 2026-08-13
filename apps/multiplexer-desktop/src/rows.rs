//! List and inspector rows. Clip, nowrap, leaf titles.

use gpui::{div, prelude::*, px, Context, MouseButton, SharedString};

use crate::theme::Theme;
use crate::ShellView;
use multiplexer_shell::{ListRowSpec, Tone};

#[allow(clippy::too_many_arguments)]
pub fn list_row(
    id: impl Into<String>,
    icon: &'static str,
    title: impl Into<String>,
    subtitle: impl Into<String>,
    meta: impl Into<String>,
    selected: bool,
    busy: bool,
    cx: &mut Context<ShellView>,
    on_click: impl Fn(&mut ShellView, &mut Context<ShellView>) + 'static,
) -> gpui::AnyElement {
    let title = title.into();
    let subtitle = subtitle.into();
    let meta = meta.into();
    let id = id.into();
    let menu_id = id.clone();
    div()
        .id(SharedString::from(id))
        .mx_2()
        .mb_1()
        .h(Theme::row_height())
        .px_2()
        .overflow_hidden()
        .rounded_lg()
        .bg(if selected {
            Theme::selection()
        } else {
            Theme::glass_ultra()
        })
        .border_1()
        .border_color(if selected {
            Theme::hairline_bright()
        } else {
            Theme::hairline()
        })
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| on_click(this, cx)),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, _, _, cx| {
                this.open_row_menu(&menu_id);
                cx.notify();
            }),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .overflow_hidden()
                .child(div().text_color(Theme::accent()).child(icon))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(
                    div()
                        .text_color(Theme::faint())
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(if busy { "…".to_owned() } else { meta }),
                ),
        )
        .child(if subtitle.is_empty() {
            div()
        } else {
            div()
                .text_size(px(11.0))
                .text_color(Theme::muted())
                .overflow_hidden()
                .whitespace_nowrap()
                .child(subtitle)
        })
        .into_any()
}

pub fn inspector_row_el(
    row: ListRowSpec,
    detail: String,
    cx: &mut Context<ShellView>,
) -> gpui::AnyElement {
    let id = row.id.clone();
    let click_id = id.clone();
    let menu_id = id.clone();
    let title = row.title.clone();
    let subtitle = row.subtitle.clone();
    let meta = row.meta.clone();
    let icon = row.icon.clone();
    let selected = row.selected || row.expanded;
    let expanded = row.expanded;
    let badge = row.badge.clone();
    let body = if !detail.is_empty() { detail } else { meta };
    div()
        .id(SharedString::from(id.clone()))
        .mx_2()
        .mb_1()
        .min_h(Theme::row_height())
        .px_2()
        .py_1()
        .overflow_hidden()
        .rounded_lg()
        .bg(if selected {
            Theme::selection()
        } else {
            Theme::glass_ultra()
        })
        .border_1()
        .border_color(if selected {
            Theme::hairline_bright()
        } else {
            Theme::hairline()
        })
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                this.activate_inspector_row(&click_id);
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, _, _, cx| {
                this.open_row_menu(&menu_id);
                cx.notify();
            }),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .items_center()
                .child(div().text_color(Theme::accent()).child(icon))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(if let Some(b) = badge {
                    let tone_bg = match b.tone {
                        Tone::Warn => Theme::warn(),
                        Tone::Danger => Theme::danger(),
                        Tone::Good => Theme::good(),
                        _ => Theme::accent_muted(),
                    };
                    div()
                        .px_1()
                        .rounded_md()
                        .bg(tone_bg)
                        .text_color(Theme::text())
                        .child(b.text)
                } else {
                    div()
                }),
        )
        .child(if subtitle.is_empty() {
            div()
        } else {
            div()
                .text_color(Theme::muted())
                .overflow_hidden()
                .whitespace_nowrap()
                .child(subtitle)
        })
        .child(if expanded && !body.is_empty() {
            div()
                .mt_1()
                .px_2()
                .py_1()
                .rounded_lg()
                .bg(Theme::wash_soft())
                .text_color(Theme::faint())
                .child(body)
        } else {
            div()
        })
        .into_any()
}
