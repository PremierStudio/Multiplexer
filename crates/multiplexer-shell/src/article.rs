//! Article blocks for the center chat. Headless, no GPUI.

/// One block in an assistant (or user) article.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArticleBlock {
    Heading { level: u8, text: String },
    Paragraph(String),
    Bullet(String),
    Numbered { n: u32, text: String },
    Quote(String),
    Fence { lang: String, body: String },
}

/// Inline run inside a paragraph or heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineSpan {
    Text(String),
    Code(String),
    Strong(String),
}

/// Split markdown-ish text into article blocks.
pub fn parse_article(src: &str) -> Vec<ArticleBlock> {
    let mut out = Vec::new();
    let mut lines = src.lines().peekable();
    let mut para: Vec<String> = Vec::new();
    let flush_para = |para: &mut Vec<String>, out: &mut Vec<ArticleBlock>| {
        if para.is_empty() {
            return;
        }
        let text = para.join(" ");
        para.clear();
        if !text.trim().is_empty() {
            out.push(ArticleBlock::Paragraph(text));
        }
    };
    while let Some(line) = lines.next() {
        if let Some(rest) = line.strip_prefix("```") {
            flush_para(&mut para, &mut out);
            let lang = rest.trim().to_owned();
            let mut body = String::new();
            for inner in lines.by_ref() {
                if inner.starts_with("```") {
                    break;
                }
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(inner);
            }
            out.push(ArticleBlock::Fence { lang, body });
            continue;
        }
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            flush_para(&mut para, &mut out);
            continue;
        }
        if let Some(text) = heading_text(trimmed) {
            flush_para(&mut para, &mut out);
            out.push(ArticleBlock::Heading {
                level: text.0,
                text: text.1,
            });
            continue;
        }
        if let Some(text) = trimmed.strip_prefix("> ") {
            flush_para(&mut para, &mut out);
            out.push(ArticleBlock::Quote(text.to_owned()));
            continue;
        }
        if let Some(text) = bullet_text(trimmed) {
            flush_para(&mut para, &mut out);
            out.push(ArticleBlock::Bullet(text));
            continue;
        }
        if let Some((n, text)) = numbered_text(trimmed) {
            flush_para(&mut para, &mut out);
            out.push(ArticleBlock::Numbered { n, text });
            continue;
        }
        para.push(trimmed.trim().to_owned());
    }
    flush_para(&mut para, &mut out);
    if out.is_empty() && !src.trim().is_empty() {
        out.push(ArticleBlock::Paragraph(src.trim().to_owned()));
    }
    out
}

/// Split `**bold**` and `` `code` `` out of a line.
pub fn parse_inlines(src: &str) -> Vec<InlineSpan> {
    let mut out = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut Vec<InlineSpan>| {
        if !buf.is_empty() {
            out.push(InlineSpan::Text(std::mem::take(buf)));
        }
    };
    while i < chars.len() {
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '`') {
                flush(&mut buf, &mut out);
                let code: String = chars[i + 1..i + 1 + end].iter().collect();
                out.push(InlineSpan::Code(code));
                i += end + 2;
                continue;
            }
        }
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(end) = chars[i + 2..].windows(2).position(|w| w == ['*', '*']) {
                flush(&mut buf, &mut out);
                let strong: String = chars[i + 2..i + 2 + end].iter().collect();
                out.push(InlineSpan::Strong(strong));
                i += end + 4;
                continue;
            }
        }
        buf.push(chars[i]);
        i += 1;
    }
    flush(&mut buf, &mut out);
    if out.is_empty() {
        out.push(InlineSpan::Text(src.to_owned()));
    }
    out
}

fn heading_text(line: &str) -> Option<(u8, String)> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let mut n = 0u8;
    for c in t.chars() {
        if c == '#' && n < 3 {
            n += 1;
        } else {
            break;
        }
    }
    if n == 0 {
        return None;
    }
    let rest = t.get(n as usize..)?.trim();
    if rest.is_empty() {
        return None;
    }
    Some((n, rest.to_owned()))
}

fn bullet_text(line: &str) -> Option<String> {
    let t = line.trim_start();
    t.strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .map(str::to_owned)
}

fn numbered_text(line: &str) -> Option<(u32, String)> {
    let t = line.trim_start();
    let (num, rest) = t.split_once(". ")?;
    let n = num.parse::<u32>().ok()?;
    if n == 0 {
        return None;
    }
    Some((n, rest.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_headings_lists_and_fence() {
        let blocks = parse_article(
            "# Report\n\nHello **world** and `src/lib.rs`.\n\n- New file\n- Modified\n\n1. First\n2. Second\n\n```rs\nfn x() {}\n```\n\n> quoted\n",
        );
        assert!(matches!(&blocks[0], ArticleBlock::Heading { level: 1, text } if text == "Report"));
        assert!(matches!(&blocks[1], ArticleBlock::Paragraph(p) if p.contains("Hello")));
        assert!(matches!(&blocks[2], ArticleBlock::Bullet(t) if t == "New file"));
        assert!(matches!(&blocks[3], ArticleBlock::Bullet(t) if t == "Modified"));
        assert!(matches!(&blocks[4], ArticleBlock::Numbered { n: 1, .. }));
        assert!(matches!(&blocks[5], ArticleBlock::Numbered { n: 2, .. }));
        assert!(
            matches!(&blocks[6], ArticleBlock::Fence { lang, body } if lang == "rs" && body.contains("fn x"))
        );
        assert!(matches!(&blocks[7], ArticleBlock::Quote(t) if t == "quoted"));
    }

    #[test]
    fn inlines_split_code_and_strong() {
        let spans = parse_inlines("see `alpha.rs` and **bold** text");
        assert_eq!(
            spans,
            vec![
                InlineSpan::Text("see ".into()),
                InlineSpan::Code("alpha.rs".into()),
                InlineSpan::Text(" and ".into()),
                InlineSpan::Strong("bold".into()),
                InlineSpan::Text(" text".into()),
            ]
        );
        assert_eq!(
            parse_inlines("plain"),
            vec![InlineSpan::Text("plain".into())]
        );
        assert!(parse_article("").is_empty());
        assert_eq!(
            parse_article("just a line"),
            vec![ArticleBlock::Paragraph("just a line".into())]
        );
    }
}
