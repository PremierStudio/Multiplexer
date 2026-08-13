//! GPUI projection of [`multiplexer_shell::parse_article`].

use gpui::{div, prelude::*, px, SharedString};

use crate::theme::Theme;
use multiplexer_shell::{parse_article, parse_inlines, ArticleBlock, InlineSpan};

pub fn render_article(text: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .w_full()
        .children(parse_article(text).into_iter().map(render_block))
        .into_any()
}

fn render_block(block: ArticleBlock) -> gpui::AnyElement {
    match block {
        ArticleBlock::Heading { level, text } => div()
            .mt_2()
            .text_size(if level <= 1 { px(18.0) } else { px(15.0) })
            .text_color(Theme::text())
            .child(render_inlines(&text))
            .into_any(),
        ArticleBlock::Paragraph(text) => div()
            .text_size(px(14.0))
            .text_color(Theme::text())
            .child(render_inlines(&text))
            .into_any(),
        ArticleBlock::Bullet(text) => div()
            .flex()
            .gap_2()
            .child(div().text_color(Theme::faint()).child("·"))
            .child(render_inlines(&text))
            .into_any(),
        ArticleBlock::Numbered { n, text } => div()
            .flex()
            .gap_2()
            .child(div().text_color(Theme::faint()).child(format!("{n}.")))
            .child(render_inlines(&text))
            .into_any(),
        ArticleBlock::Quote(text) => div()
            .pl_3()
            .border_l_1()
            .border_color(Theme::hairline())
            .text_color(Theme::muted())
            .child(render_inlines(&text))
            .into_any(),
        ArticleBlock::Fence { lang, body } => div()
            .mt_1()
            .px_3()
            .py_2()
            .rounded_lg()
            .bg(Theme::raised())
            .text_size(Theme::text_caption())
            .text_color(Theme::muted())
            .child(if lang.is_empty() {
                SharedString::from(body)
            } else {
                SharedString::from(format!("{lang}\n{body}"))
            })
            .into_any(),
    }
}

fn render_inlines(text: &str) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .children(parse_inlines(text).into_iter().map(|span| {
            match span {
                InlineSpan::Text(t) => div().child(t).into_any(),
                InlineSpan::Code(t) => div()
                    .px_1()
                    .rounded_lg()
                    .bg(Theme::raised())
                    .text_color(Theme::good())
                    .child(t)
                    .into_any(),
                InlineSpan::Strong(t) => div().text_color(Theme::text()).child(t).into_any(),
            }
        }))
}
