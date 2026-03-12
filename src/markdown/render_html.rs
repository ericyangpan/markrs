use crate::{
    RenderOptions,
    markdown::{parser, render},
};

pub(crate) fn render_markdown_to_html(input: &str, options: RenderOptions) -> String {
    if let Some(html) = try_fast_render(input, options) {
        return html;
    }
    let document = parser::parse_document(input, options);
    render::render_document(&document, options, input.len())
}

pub(crate) fn render_markdown_to_html_buf(input: &str, options: RenderOptions, buf: &mut String) {
    if try_fast_render_into(input, options, buf) {
        return;
    }
    let document = parser::parse_document(input, options);
    render::render_document_into(&document, options, buf);
}

/// Fast path for trivial single-paragraph documents with no inline syntax.
/// Returns None if the input requires full parsing.
#[inline]
fn try_fast_render(input: &str, options: RenderOptions) -> Option<String> {
    if !is_trivial_paragraph(input, options) {
        return None;
    }
    let trimmed = input.trim();
    let mut out = String::with_capacity(trimmed.len() + 8);
    out.push_str("<p>");
    render::escape_html_to_pub(trimmed, &mut out);
    out.push_str("</p>\n");
    Some(out)
}

#[inline]
fn try_fast_render_into(input: &str, options: RenderOptions, buf: &mut String) -> bool {
    if !is_trivial_paragraph(input, options) {
        return false;
    }
    let trimmed = input.trim();
    buf.push_str("<p>");
    render::escape_html_to_pub(trimmed, buf);
    buf.push_str("</p>\n");
    true
}

/// A trivial paragraph is a single non-empty line with no block or inline syntax.
#[inline]
fn is_trivial_paragraph(input: &str, options: RenderOptions) -> bool {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    // Must not start with whitespace that could form indented code (4+ columns)
    let mut indent_cols = 0u8;
    for &b in bytes {
        match b {
            b' ' => {
                indent_cols += 1;
                if indent_cols >= 4 {
                    return false;
                }
            }
            b'\t' => return false, // tab always reaches 4+ columns
            _ => break,
        }
    }
    // Scan for any character that requires full parsing
    for &b in bytes {
        match b {
            b'\n' | b'\r' => return false, // multi-line
            b'\\' | b'*' | b'_' | b'[' | b']' | b'!' | b'`' | b'~' | b'<' | b'>'
            | b'#' | b'-' | b'+' | b'=' | b'|' | b'&' => return false,
            // GFM bare autolinks trigger on ':', '@', '.'
            b':' | b'@' | b'.' if options.gfm => return false,
            _ => {}
        }
    }
    true
}
