use std::fmt::Write as _;

use crate::RenderOptions;
use crate::markdown::ast::{self, inline::Inline};

pub(crate) fn render_document(doc: &ast::Document, options: RenderOptions) -> String {
    let mut out = String::new();

    match doc {
        ast::Document::Nodes(blocks) => {
            for block in blocks {
                render_block(block, &mut out, options);
            }
        }
    }
    out
}

fn render_block(block: &ast::Block, out: &mut String, options: RenderOptions) {
    match block {
        ast::Block::Paragraph { inlines } => {
            out.push_str("<p>");
            render_inlines(inlines, out, options);
            out.push_str("</p>\n");
        }
        ast::Block::Heading { level, inlines } => {
            write!(out, "<h{level}>").expect("write heading");
            render_inlines(inlines, out, options);
            write!(out, "</h{level}>\n").expect("write heading close");
        }
        ast::Block::List {
            ordered,
            start,
            tight,
            items,
        } => {
            if *ordered {
                if *start == 1 {
                    out.push_str("<ol>");
                } else {
                    write!(out, "<ol start=\"{start}\">").expect("write ordered list start");
                }
            } else {
                out.push_str("<ul>");
            }
            for item in items {
                out.push_str("<li>");
                let children = &item.children;
                if *tight {
                    render_tight_list_item(children, item.task, out, options);
                    out.push_str("</li>");
                    continue;
                }

                let mut rendered_task_paragraph = false;
                if let Some(done) = item.task {
                    if let Some(ast::Block::Paragraph { inlines }) = children.first() {
                        out.push_str("<p>");
                        render_task_checkbox(done, out);
                        if !inlines.is_empty() {
                            out.push(' ');
                        }
                        render_inlines(inlines, out, options);
                        out.push_str("</p>\n");
                        rendered_task_paragraph = true;
                    } else if let Some(ast::Block::HtmlBlock(raw)) = children.first() {
                        out.push_str("<p>");
                        render_task_checkbox(done, out);
                        out.push(' ');
                        render_raw_html(raw, out, options);
                        out.push_str("</p>\n");
                        rendered_task_paragraph = true;
                    } else {
                        render_task_checkbox(done, out);
                    }
                }

                for (idx, child) in children.iter().enumerate() {
                    if rendered_task_paragraph && idx == 0 {
                        continue;
                    }
                    render_block(child, out, options);
                }
                out.push_str("</li>");
            }
            if *ordered {
                out.push_str("</ol>\n");
            } else {
                out.push_str("</ul>\n");
            }
        }
        ast::Block::BlockQuote { children } => {
            out.push_str("<blockquote>");
            for child in children {
                render_block(child, out, options);
            }
            out.push_str("</blockquote>\n");
        }
        ast::Block::CodeBlock { info, content } => {
            if let Some(language) = info
                .as_deref()
                .and_then(extract_code_block_language)
                .map(unescape_code_block_language)
            {
                if !language.is_empty() {
                    out.push_str("<pre><code class=\"language-");
                    out.push_str(&escape_html(&language));
                    out.push_str("\">");
                    out.push_str(&escape_html(content));
                    out.push_str("</code></pre>\n");
                    return;
                }
            }
            out.push_str("<pre><code>");
            out.push_str(&escape_html(content));
            out.push_str("</code></pre>\n");
        }
        ast::Block::ThematicBreak => {
            out.push_str("<hr>\n");
        }
        ast::Block::Table {
            aligns,
            header,
            rows,
        } => {
            out.push_str("<table><thead><tr>");
            for (idx, cell) in header.iter().enumerate() {
                out.push_str("<th");
                render_table_align_attr(aligns.get(idx).copied().flatten(), out);
                out.push('>');
                render_inlines(cell, out, options);
                out.push_str("</th>");
            }
            out.push_str("</tr></thead>");
            if !rows.is_empty() {
                out.push_str("<tbody>");
                for row in rows {
                    out.push_str("<tr>");
                    for (idx, cell) in row.iter().enumerate() {
                        out.push_str("<td");
                        render_table_align_attr(aligns.get(idx).copied().flatten(), out);
                        out.push('>');
                        render_inlines(cell, out, options);
                        out.push_str("</td>");
                    }
                    out.push_str("</tr>");
                }
                out.push_str("</tbody>");
            }
            out.push_str("</table>\n");
        }
        ast::Block::HtmlBlock(raw) => {
            render_raw_html(raw, out, options);
            out.push('\n');
        }
    }
}

fn render_task_checkbox(done: bool, out: &mut String) {
    out.push_str("<input type=\"checkbox\"");
    if done {
        out.push_str(" checked=\"\"");
    }
    out.push_str(" disabled=\"\">");
}

fn render_tight_list_item(
    children: &[ast::Block],
    task: Option<bool>,
    out: &mut String,
    options: RenderOptions,
) {
    for (idx, child) in children.iter().enumerate() {
        if idx > 0 {
            render_tight_list_separator(&children[idx - 1], child, out);
        }

        match child {
            ast::Block::Paragraph { inlines } => {
                if idx == 0 {
                    if let Some(done) = task {
                        render_task_checkbox(done, out);
                        if !inlines.is_empty() {
                            out.push(' ');
                        }
                    }
                }
                render_inlines(inlines, out, options);
            }
            _ => render_block(child, out, options),
        }
    }

    if matches!(children.last(), Some(ast::Block::HtmlBlock(_))) && out.ends_with('\n') {
        out.pop();
    }
}

fn render_tight_list_separator(
    prev_child: &ast::Block,
    next_child: &ast::Block,
    _out: &mut String,
) {
    match (prev_child, next_child) {
        (ast::Block::Paragraph { .. }, _) => {}
        _ => {}
    }
}

fn render_inlines(inlines: &[Inline], out: &mut String, options: RenderOptions) {
    for inline in inlines {
        match inline {
            Inline::Text(text) => out.push_str(&escape_html(text)),
            Inline::RawHtml(html) => render_raw_html(html, out, options),
            Inline::SoftBreak => {
                if options.breaks {
                    out.push_str("<br>\n");
                } else {
                    out.push('\n');
                }
            }
            Inline::HardBreak => out.push_str("<br>\n"),
            Inline::Code(text) => {
                out.push_str("<code>");
                out.push_str(&escape_html(text));
                out.push_str("</code>");
            }
            Inline::Em(children) => {
                out.push_str("<em>");
                render_inlines(children, out, options);
                out.push_str("</em>");
            }
            Inline::Strong(children) => {
                out.push_str("<strong>");
                render_inlines(children, out, options);
                out.push_str("</strong>");
            }
            Inline::Del(children) => {
                out.push_str("<del>");
                render_inlines(children, out, options);
                out.push_str("</del>");
            }
            Inline::Link { label, href, title } => {
                out.push_str("<a href=\"");
                out.push_str(&escape_href_attr(href));
                out.push('"');
                if let Some(title) = title {
                    out.push_str(" title=\"");
                    out.push_str(&escape_html_attr(title));
                    out.push('"');
                }
                out.push('>');
                render_inlines(label, out, options);
                out.push_str("</a>");
            }
            Inline::Image { alt, src, title } => {
                out.push_str("<img src=\"");
                out.push_str(&escape_html_attr(src));
                out.push_str("\" alt=\"");
                out.push_str(&escape_html_attr(&inline_text_content(alt)));
                out.push('"');
                if let Some(title) = title {
                    out.push_str(" title=\"");
                    out.push_str(&escape_html_attr(title));
                    out.push('"');
                }
                out.push('>');
            }
        }
    }
}

fn render_raw_html(raw: &str, out: &mut String, options: RenderOptions) {
    if options.gfm {
        out.push_str(&escape_disallowed_raw_html(raw));
        return;
    }
    out.push_str(raw);
}

fn escape_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn extract_code_block_language(info: &str) -> Option<&str> {
    info.split_whitespace().next()
}

fn unescape_code_block_language(language: &str) -> String {
    let mut out = String::with_capacity(language.len());
    let mut chars = language.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if next.is_ascii_punctuation() {
                    out.push(next);
                } else {
                    out.push('\\');
                    out.push(next);
                }
                continue;
            }
        }
        out.push(ch);
    }

    out
}

fn escape_html_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_href_attr(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("%22"),
            _ => out.push(ch),
        }
    }
    out
}

fn render_table_align_attr(align: Option<ast::TableAlignment>, out: &mut String) {
    match align {
        Some(ast::TableAlignment::Left) => out.push_str(" align=\"left\""),
        Some(ast::TableAlignment::Center) => out.push_str(" align=\"center\""),
        Some(ast::TableAlignment::Right) => out.push_str(" align=\"right\""),
        None => {}
    }
}

fn inline_text_content(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(text) | Inline::Code(text) | Inline::RawHtml(text) => out.push_str(text),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
            Inline::Em(children) | Inline::Strong(children) | Inline::Del(children) => {
                out.push_str(&inline_text_content(children));
            }
            Inline::Link { label, .. } => out.push_str(&inline_text_content(label)),
            Inline::Image { alt, .. } => out.push_str(&inline_text_content(alt)),
        }
    }
    out
}

fn escape_disallowed_raw_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;

    while i < raw.len() {
        let tail = &raw[i..];
        let Some(offset) = tail.find('<') else {
            out.push_str(tail);
            break;
        };
        out.push_str(&tail[..offset]);
        i += offset;

        let Some(tag_end_offset) = raw[i..].find('>') else {
            out.push_str(&raw[i..]);
            break;
        };
        let tag_end = i + tag_end_offset + 1;
        let tag = &raw[i..tag_end];

        if is_disallowed_raw_html_tag(tag) {
            out.push_str("&lt;");
            out.push_str(&tag['<'.len_utf8()..]);
        } else {
            out.push_str(tag);
        }
        i = tag_end;
    }

    out
}

fn is_disallowed_raw_html_tag(tag: &str) -> bool {
    let trimmed = tag.trim();
    if !trimmed.starts_with('<') {
        return false;
    }

    let Some(name) = parse_tag_name(trimmed.trim_start_matches('<').trim_end_matches('>')) else {
        return false;
    };

    matches!(
        name.as_str(),
        "title"
            | "textarea"
            | "style"
            | "xmp"
            | "iframe"
            | "noembed"
            | "noframes"
            | "script"
            | "plaintext"
    )
}

fn parse_tag_name(tag_body: &str) -> Option<String> {
    let mut chars = tag_body.chars().peekable();
    while matches!(chars.peek(), Some(c) if c.is_whitespace()) {
        chars.next();
    }

    if matches!(chars.peek(), Some('/')) {
        chars.next();
    }

    let mut name = String::new();
    while let Some(c) = chars.peek().copied() {
        if c.is_whitespace() || c == '/' || c == '>' {
            break;
        }
        name.push(c.to_ascii_lowercase());
        chars.next();
    }

    if name.is_empty() { None } else { Some(name) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_image_title_without_duplication() {
        let node = ast::Block::Paragraph {
            inlines: vec![ast::inline::Inline::Image {
                alt: vec![ast::inline::Inline::Text("logo".to_string())],
                src: "logo.png".to_string(),
                title: Some("Markec logo".to_string()),
            }],
        };

        let mut out = String::new();
        render_block(&node, &mut out, RenderOptions::default());
        assert_eq!(
            out,
            "<p><img src=\"logo.png\" alt=\"logo\" title=\"Markec logo\"></p>\n"
        );
    }

    #[test]
    fn renders_ordered_list_start_when_non_one() {
        let node = ast::Block::List {
            ordered: true,
            start: 2,
            tight: true,
            items: vec![ast::ListItem {
                children: vec![ast::Block::Paragraph {
                    inlines: vec![ast::inline::Inline::Text("two".to_string())],
                }],
                task: None,
            }],
        };

        let mut out = String::new();
        render_block(&node, &mut out, RenderOptions::default());
        assert_eq!(out, "<ol start=\"2\"><li>two</li></ol>\n");
    }

    #[test]
    fn renders_tight_list_without_separator_before_following_blocks() {
        let node = ast::Block::List {
            ordered: false,
            start: 1,
            tight: true,
            items: vec![ast::ListItem {
                children: vec![
                    ast::Block::Paragraph {
                        inlines: vec![ast::inline::Inline::Text("list".to_string())],
                    },
                    ast::Block::Heading {
                        level: 1,
                        inlines: vec![ast::inline::Inline::Text("header".to_string())],
                    },
                ],
                task: None,
            }],
        };

        let mut out = String::new();
        render_block(&node, &mut out, RenderOptions::default());
        assert_eq!(out, "<ul><li>list<h1>header</h1>\n</li></ul>\n");
    }
}
