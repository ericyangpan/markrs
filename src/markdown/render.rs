use std::fmt::Write as _;

use crate::RenderOptions;
use crate::markdown::{
    ast::{self, inline::Inline},
    block::parse_html_entity,
};

#[derive(Clone, Copy)]
struct HtmlTextAtom {
    raw_start: usize,
    raw_end: usize,
    ch: char,
}

struct AutolinkCandidate {
    end: usize,
    href: String,
}

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
        ast::Block::ListItem { .. } => {}
        ast::Block::BlockQuote { children } => {
            out.push_str("<blockquote>");
            for child in children {
                render_block(child, out, options);
            }
            out.push_str("</blockquote>\n");
        }
        ast::Block::CodeBlock {
            info,
            content,
            fenced: _,
        } => {
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
}

fn render_tight_list_separator(prev_child: &ast::Block, next_child: &ast::Block, out: &mut String) {
    match (prev_child, next_child) {
        (ast::Block::Paragraph { .. }, ast::Block::Heading { .. }) => out.push(' '),
        (ast::Block::Paragraph { .. }, _) => out.push('\n'),
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

pub(crate) fn post_autolink(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    let bytes = input.as_bytes();
    let mut stack: Vec<String> = Vec::new();

    while i < input.len() {
        if bytes[i] == b'<' {
            let tag_end = input[i..]
                .find('>')
                .map(|offset| i + offset + 1)
                .unwrap_or(input.len());
            let tag = &input[i..tag_end];
            update_html_stack(tag, &mut stack);
            out.push_str(tag);
            i = tag_end;
            continue;
        }

        let next_tag = input[i..]
            .find('<')
            .map(|offset| i + offset)
            .unwrap_or(input.len());
        let text = &input[i..next_tag];
        if should_skip_autolink(&stack) {
            out.push_str(text);
        } else {
            out.push_str(&autolink_text_segment(text));
        }
        i = next_tag;
    }

    out
}

fn should_skip_autolink(stack: &[String]) -> bool {
    stack.iter().any(|tag| {
        matches!(
            tag.as_str(),
            "a" | "code" | "pre" | "script" | "style" | "textarea"
        )
    })
}

fn autolink_text_segment(text: &str) -> String {
    let atoms = html_text_atoms(text);
    let mut out = String::with_capacity(text.len());
    let mut last_raw = 0usize;
    let mut i = 0usize;

    while i < atoms.len() {
        let Some(candidate) = parse_autolink_candidate(&atoms, i) else {
            i += 1;
            continue;
        };

        let raw_start = atoms[i].raw_start;
        let raw_end = atoms[candidate.end - 1].raw_end;
        out.push_str(&text[last_raw..raw_start]);
        out.push_str("<a href=\"");
        out.push_str(&escape_href_attr(&candidate.href));
        out.push_str("\">");
        out.push_str(&text[raw_start..raw_end]);
        out.push_str("</a>");
        last_raw = raw_end;
        i = candidate.end;
    }

    out.push_str(&text[last_raw..]);
    out
}

fn html_text_atoms(text: &str) -> Vec<HtmlTextAtom> {
    let mut atoms = Vec::with_capacity(text.len());
    let mut i = 0usize;

    while i < text.len() {
        let tail = &text[i..];
        if let Some((decoded, consumed)) = parse_html_entity(tail) {
            let raw_end = i + consumed;
            for ch in decoded.chars() {
                atoms.push(HtmlTextAtom {
                    raw_start: i,
                    raw_end,
                    ch,
                });
            }
            i = raw_end;
            continue;
        }

        let Some(ch) = tail.chars().next() else {
            break;
        };
        let raw_end = i + ch.len_utf8();
        atoms.push(HtmlTextAtom {
            raw_start: i,
            raw_end,
            ch,
        });
        i = raw_end;
    }

    atoms
}

fn parse_autolink_candidate(atoms: &[HtmlTextAtom], start: usize) -> Option<AutolinkCandidate> {
    if !autolink_start_boundary(atoms, start) {
        return None;
    }

    parse_url_candidate(atoms, start).or_else(|| parse_email_candidate(atoms, start))
}

fn autolink_start_boundary(atoms: &[HtmlTextAtom], start: usize) -> bool {
    if start == 0 {
        return true;
    }
    !matches!(
        atoms[start - 1].ch,
        'a'..='z' | 'A'..='Z' | '0'..='9' | '@' | '.' | '_' | '-' | '/' | ':'
    )
}

fn parse_url_candidate(atoms: &[HtmlTextAtom], start: usize) -> Option<AutolinkCandidate> {
    if starts_with_www(atoms, start) {
        let end = trim_generic_url_end(atoms, start, scan_url_end(atoms, start));
        if end <= start + 4 {
            return None;
        }
        return Some(AutolinkCandidate {
            end,
            href: format!("http://{}", collect_decoded(atoms, start, end)),
        });
    }

    let (scheme_end, scheme) = parse_scheme_prefix(atoms, start)?;
    if scheme_end >= atoms.len() || atoms[scheme_end].ch.is_whitespace() {
        return None;
    }

    if scheme.eq_ignore_ascii_case("mailto") || scheme.eq_ignore_ascii_case("xmpp") {
        return parse_emailish_scheme_candidate(atoms, start, scheme_end + 1, &scheme);
    }

    let end = trim_generic_url_end(atoms, start, scan_url_end(atoms, start));
    if end <= scheme_end + 1 {
        return None;
    }

    Some(AutolinkCandidate {
        end,
        href: collect_decoded(atoms, start, end),
    })
}

fn parse_emailish_scheme_candidate(
    atoms: &[HtmlTextAtom],
    start: usize,
    body_start: usize,
    scheme: &str,
) -> Option<AutolinkCandidate> {
    let email_end = parse_email_body(atoms, body_start)?;
    let mut end = email_end;

    if scheme.eq_ignore_ascii_case("xmpp") && end < atoms.len() && atoms[end].ch == '/' {
        let mut path_end = end + 1;
        while path_end < atoms.len() && is_email_path_char(atoms[path_end].ch) {
            path_end += 1;
        }
        if path_end > end + 1 {
            end = path_end;
        }
    }

    if matches!(atoms.get(end).map(|atom| atom.ch), Some('-' | '_')) {
        return None;
    }

    Some(AutolinkCandidate {
        end,
        href: collect_decoded(atoms, start, end),
    })
}

fn parse_email_candidate(atoms: &[HtmlTextAtom], start: usize) -> Option<AutolinkCandidate> {
    let end = parse_email_body(atoms, start)?;
    if matches!(atoms.get(end).map(|atom| atom.ch), Some('-' | '_')) {
        return None;
    }

    Some(AutolinkCandidate {
        end,
        href: format!("mailto:{}", collect_decoded(atoms, start, end)),
    })
}

fn parse_email_body(atoms: &[HtmlTextAtom], start: usize) -> Option<usize> {
    let mut i = start;
    while i < atoms.len() && is_email_local_char(atoms[i].ch) {
        i += 1;
    }
    if i == start || i >= atoms.len() || atoms[i].ch != '@' {
        return None;
    }
    i += 1;

    let mut labels = 0usize;
    loop {
        let label_start = i;
        while i < atoms.len() && is_domain_label_char(atoms[i].ch) {
            i += 1;
        }
        if i == label_start {
            return None;
        }
        if atoms[label_start].ch == '-' || atoms[i - 1].ch == '-' {
            return None;
        }
        labels += 1;
        if i < atoms.len() && atoms[i].ch == '.' {
            if i + 1 >= atoms.len() || !is_domain_label_char(atoms[i + 1].ch) {
                break;
            }
            i += 1;
            continue;
        }
        break;
    }

    if labels < 2 {
        return None;
    }

    Some(i)
}

fn starts_with_www(atoms: &[HtmlTextAtom], start: usize) -> bool {
    matches!(
        (
            atoms.get(start).map(|atom| atom.ch),
            atoms.get(start + 1).map(|atom| atom.ch),
            atoms.get(start + 2).map(|atom| atom.ch),
            atoms.get(start + 3).map(|atom| atom.ch),
        ),
        (Some('w'), Some('w'), Some('w'), Some('.'))
    )
}

fn parse_scheme_prefix(atoms: &[HtmlTextAtom], start: usize) -> Option<(usize, String)> {
    let first = atoms.get(start)?.ch;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let mut i = start + 1;
    while i < atoms.len()
        && (atoms[i].ch.is_ascii_alphanumeric() || matches!(atoms[i].ch, '+' | '-' | '.'))
    {
        i += 1;
    }
    if i >= atoms.len() || atoms[i].ch != ':' {
        return None;
    }

    let len = i - start;
    if !(2..=32).contains(&len) {
        return None;
    }

    Some((i, collect_decoded(atoms, start, i)))
}

fn scan_url_end(atoms: &[HtmlTextAtom], start: usize) -> usize {
    let mut end = start;
    while end < atoms.len() {
        let ch = atoms[end].ch;
        if ch.is_whitespace() || ch == '<' {
            break;
        }
        end += 1;
    }
    end
}

fn trim_generic_url_end(atoms: &[HtmlTextAtom], start: usize, mut end: usize) -> usize {
    loop {
        if end <= start {
            return end;
        }

        let last = atoms[end - 1].ch;
        let mut trimmed = false;

        if matches!(last, '.' | ',' | ':' | '!' | '?' | '"' | '\'') {
            end -= 1;
            trimmed = true;
        } else if last == ';' {
            if let Some(entity_start) = entity_like_suffix_start(atoms, start, end) {
                end = entity_start;
            } else {
                end -= 1;
            }
            trimmed = true;
        } else if last == ')' && unmatched_closing_parens(atoms, start, end) > 0 {
            end -= 1;
            trimmed = true;
        }

        if !trimmed {
            break;
        }
    }

    end
}

fn entity_like_suffix_start(atoms: &[HtmlTextAtom], start: usize, end: usize) -> Option<usize> {
    if end <= start || atoms[end - 1].ch != ';' {
        return None;
    }

    let mut i = end - 1;
    while i > start && atoms[i - 1].ch.is_ascii_alphanumeric() {
        i -= 1;
    }
    if i > start && atoms[i - 1].ch == '&' && i < end - 1 {
        return Some(i - 1);
    }
    None
}

fn unmatched_closing_parens(atoms: &[HtmlTextAtom], start: usize, end: usize) -> usize {
    let opens = atoms[start..end]
        .iter()
        .filter(|atom| atom.ch == '(')
        .count();
    let closes = atoms[start..end]
        .iter()
        .filter(|atom| atom.ch == ')')
        .count();
    closes.saturating_sub(opens)
}

fn collect_decoded(atoms: &[HtmlTextAtom], start: usize, end: usize) -> String {
    atoms[start..end].iter().map(|atom| atom.ch).collect()
}

fn is_email_local_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '+' | '-')
}

fn is_domain_label_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-'
}

fn is_email_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '+' | '-' | '@')
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

fn is_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area"
            | "base"
            | "br"
            | "col"
            | "embed"
            | "hr"
            | "img"
            | "input"
            | "link"
            | "meta"
            | "param"
            | "source"
            | "track"
            | "wbr"
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

fn update_html_stack(tag: &str, stack: &mut Vec<String>) {
    if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("<?") {
        return;
    }

    let Some(name) = parse_tag_name(tag.trim_start_matches('<').trim_end_matches('>')) else {
        return;
    };

    let trimmed = tag.trim();
    let is_end_tag = trimmed.starts_with("</");
    let self_closing = trimmed.ends_with("/>");

    if is_end_tag {
        if let Some(pos) = stack.iter().rposition(|n| n == &name) {
            stack.drain(pos..);
        }
        return;
    }

    if !self_closing && !is_void_tag(&name) {
        stack.push(name);
    }
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
}
