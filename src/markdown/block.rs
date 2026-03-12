use std::{borrow::Cow, collections::HashMap};

use crate::markdown::ast::{self, inline::Inline};
use crate::markdown::lexer::Line;

#[derive(Debug, Clone)]
pub(crate) struct ReferenceDefinition {
    pub(crate) href: String,
    pub(crate) title: Option<String>,
}

pub(crate) fn try_normalize_reference_label(label: &str) -> Option<String> {
    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    let mut pending_space = false;
    let mut escaped_for_validation = false;

    while let Some(ch) = chars.next() {
        let current_escaped = std::mem::take(&mut escaped_for_validation);
        if ch == '\\' {
            match chars.peek().copied() {
                Some('\n') => {
                    chars.next();
                    continue;
                }
                Some('\r') => {
                    chars.next();
                    if chars.peek().copied() == Some('\n') {
                        chars.next();
                    }
                    continue;
                }
                _ if !current_escaped => escaped_for_validation = true,
                _ => {}
            }
        } else if ch == '[' && !current_escaped {
            return None;
        }

        let normalized = if ch == '\\' {
            '\\'
        } else if ch == '\r' {
            if chars.peek().copied() == Some('\n') {
                chars.next();
            }
            ' '
        } else {
            ch
        };

        if normalized.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }

        if pending_space {
            out.push(' ');
            pending_space = false;
        }

        match normalized {
            'ß' | 'ẞ' => out.push_str("ss"),
            _ => out.extend(normalized.to_lowercase()),
        }
    }

    if out.is_empty() { None } else { Some(out) }
}

pub(crate) fn normalize_reference_label(label: &str) -> String {
    try_normalize_reference_label(label).unwrap_or_default()
}

#[derive(Debug, Default)]
pub(crate) struct BlockParseContext {
    refs: HashMap<String, ReferenceDefinition>,
}

impl BlockParseContext {
    pub(crate) fn new() -> Self {
        Self {
            refs: HashMap::new(),
        }
    }

    pub(crate) fn parse_lines(
        &mut self,
        lines: &[&str],
        gfm: bool,
        pedantic: bool,
    ) -> ast::Document {
        let allow_ref_defs = has_potential_reference_definition(lines);

        if allow_ref_defs {
            let mut i = 0usize;
            while i < lines.len() {
                if should_skip_prescan_ref_definition(lines, i, pedantic) {
                    i += 1;
                    continue;
                }
                if let Some((id, def, consumed)) = prescan_reference_definition(lines, i, pedantic)
                {
                    self.refs.entry(id).or_insert(def);
                    i += consumed;
                    continue;
                }
                i += 1;
            }
        }

        parse_blocks_from_lines_mode(lines, gfm, pedantic, &mut self.refs, allow_ref_defs, true)
    }

    pub(crate) fn parse_line_slices<'a>(
        &mut self,
        lines: &[Line<'a>],
        gfm: bool,
        pedantic: bool,
    ) -> ast::Document {
        let text_lines = lines.iter().map(|line| line.text).collect::<Vec<&'a str>>();
        self.parse_lines(&text_lines, gfm, pedantic)
    }
}

const FORCED_PARAGRAPH_PREFIX: char = '\u{001e}';
const LAZY_QUOTE_PREFIX: char = '\u{001f}';

fn has_potential_reference_definition(lines: &[&str]) -> bool {
    lines
        .iter()
        .copied()
        .any(line_has_potential_reference_definition)
}

fn line_has_potential_reference_definition(line: &str) -> bool {
    let mut current = line.trim_start_matches([' ', '\t']);
    while let Some(rest) = current.strip_prefix('>') {
        current = rest.trim_start_matches([' ', '\t']);
    }
    current.starts_with('[')
}

fn should_skip_prescan_ref_definition(lines: &[&str], idx: usize, pedantic: bool) -> bool {
    if idx == 0 || idx >= lines.len() {
        return false;
    }

    let current = lines[idx];
    if current.trim().is_empty() {
        return false;
    }

    let prev = lines[idx - 1];
    if prev.trim().is_empty() {
        return false;
    }

    if prescan_reference_definition(lines, idx - 1, pedantic).is_some() {
        return false;
    }

    let (_, prev_text) = split_leading_ws(prev);
    let (curr_indent, curr_text) = split_leading_ws(current);
    if let Some(prev_quote) = prev_text.strip_prefix('>') {
        let prev_quote = prev_quote.trim_start_matches([' ', '\t']);
        if !prev_quote.trim().is_empty()
            && parse_reference_definition(prev_quote, pedantic).is_none()
        {
            if curr_text.starts_with('>') || curr_indent <= 3 {
                return true;
            }
        }
    }
    if list_marker(prev_text).is_some() && list_marker(curr_text).is_none() && curr_indent <= 3 {
        return true;
    }

    if parse_atx_heading(prev, false).is_some() || parse_thematic_break(prev).is_some() {
        return false;
    }

    true
}

fn prescan_reference_definition(
    lines: &[&str],
    idx: usize,
    pedantic: bool,
) -> Option<(String, ReferenceDefinition, usize)> {
    if let Some(found) = parse_reference_definition_with_continuation(lines, idx, pedantic) {
        return Some(found);
    }

    let current = lines.get(idx)?.trim_end_matches('\r');
    let stripped = current.strip_prefix('>')?.trim_start_matches([' ', '\t']);
    let (id, def) = parse_reference_definition(stripped, pedantic)?;
    Some((id, def, 1))
}

pub(crate) fn parse_blocks_from_lines(
    lines: &[&str],
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> ast::Document {
    parse_blocks_from_lines_mode(
        lines,
        gfm,
        pedantic,
        refs,
        has_potential_reference_definition(lines),
        false,
    )
}

fn parse_blocks_from_lines_mode(
    lines: &[&str],
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
    allow_ref_defs: bool,
    preserve_paragraph_leading_indent: bool,
) -> ast::Document {
    let mut i = 0usize;
    let mut blocks = Vec::new();

    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        if let Some(block) = parse_thematic_break(line) {
            blocks.push(block);
            i += 1;
            continue;
        }

        if allow_ref_defs {
            if let Some((id, def, consumed)) =
                parse_reference_definition_with_continuation(lines, i, pedantic)
            {
                refs.entry(id).or_insert(def);
                i += consumed;
                continue;
            }
        }

        if let Some((_, quote_lines, consumed)) = parse_blockquote_block(&lines, i) {
            let quote_lines = quote_lines
                .iter()
                .map(std::string::String::as_str)
                .collect::<Vec<_>>();
            let quote_doc = parse_blocks_from_lines(&quote_lines, gfm, pedantic, refs);
            let children = match quote_doc {
                ast::Document::Nodes(nodes) => nodes,
            };
            blocks.push(ast::Block::BlockQuote { children });
            i = consumed;
            continue;
        }

        if let Some((block, consumed)) = parse_setext_heading_block(&lines, i, gfm, pedantic, refs)
        {
            blocks.push(block);
            i = consumed;
            continue;
        }

        if let Some((level, title_line)) = parse_atx_heading(line, pedantic) {
            let inlines = parse_block_inlines(title_line, gfm, pedantic, refs);
            blocks.push(ast::Block::Heading { level, inlines });
            i += 1;
            continue;
        }

        if let Some((_, info, consumed, closed, fence_indent)) = parse_fenced_code_block(&lines, i)
        {
            let content_end = if closed {
                consumed.saturating_sub(1)
            } else {
                consumed
            }
            .min(lines.len());

            let mut content = String::with_capacity(
                lines[i + 1..content_end]
                    .iter()
                    .map(|line| line.len())
                    .sum::<usize>()
                    + content_end.saturating_sub(i + 2),
            );
            for (idx, line) in lines[i + 1..content_end].iter().copied().enumerate() {
                if idx > 0 {
                    content.push('\n');
                }
                content.push_str(&strip_code_fence_indent(line, fence_indent));
            }
            content.push('\n');
            blocks.push(ast::Block::CodeBlock { info, content });
            i = consumed;
            continue;
        }

        if let Some((content, consumed)) = parse_indented_code_block(lines, i) {
            blocks.push(ast::Block::CodeBlock {
                info: None,
                content,
            });
            i = consumed;
            continue;
        }

        if let Some((ordered, start, tight, items, consumed)) =
            parse_list_block(&lines, i, gfm, pedantic, refs)
        {
            blocks.push(ast::Block::List {
                ordered,
                start,
                tight,
                items,
            });
            i = consumed;
            continue;
        }

        if let Some((block, consumed)) = parse_table_block(&lines, i, gfm, pedantic, refs) {
            blocks.push(block);
            i = consumed;
            continue;
        }

        if let Some((block, consumed)) = parse_html_block(&lines, i) {
            blocks.push(block);
            i = consumed;
            continue;
        }

        let (nodes, consumed) = parse_paragraph(
            &lines,
            i,
            gfm,
            pedantic,
            refs,
            allow_ref_defs,
            preserve_paragraph_leading_indent,
        );
        blocks.push(ast::Block::Paragraph { inlines: nodes });
        i = consumed;
    }

    ast::Document::Nodes(blocks)
}

fn parse_thematic_break(line: &str) -> Option<ast::Block> {
    let (indent, line) = split_leading_ws(line);
    if indent > 3 || line.is_empty() {
        return None;
    }

    let mut marker = 0u8;
    let mut marker_count = 0usize;
    for &c in line.as_bytes() {
        match c {
            b'-' | b'*' | b'_' => {
                if marker != 0 && marker != c {
                    return None;
                }
                marker = c;
                marker_count += 1;
            }
            b' ' | b'\t' => {}
            _ => return None,
        }
    }

    if marker_count >= 3 {
        return Some(ast::Block::ThematicBreak);
    }

    None
}

fn split_table_cells(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut cells = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut code_run_len = None;

    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run_start = i;
            while i < bytes.len() && bytes[i] == b'`' {
                i += 1;
            }
            let run_len = i - run_start;
            match code_run_len {
                Some(open_len) if open_len == run_len => code_run_len = None,
                None => code_run_len = Some(run_len),
                _ => {}
            }
            continue;
        }

        if code_run_len.is_none() && bytes[i] == b'|' && !is_escaped_pipe(line, i) {
            cells.push(line[start..i].to_string());
            start = i + 1;
        }

        i += 1;
    }

    cells.push(line[start..].to_string());

    if cells.first().is_some_and(|col| col.trim().is_empty()) {
        cells.remove(0);
    }
    if cells.last().is_some_and(|col| col.trim().is_empty()) {
        cells.pop();
    }
    cells
}

fn is_escaped_pipe(line: &str, pipe_idx: usize) -> bool {
    let bytes = line.as_bytes();
    let mut idx = pipe_idx;
    let mut backslashes = 0usize;

    while idx > 0 && bytes[idx - 1] == b'\\' {
        backslashes += 1;
        idx -= 1;
    }

    backslashes % 2 == 1
}

fn parse_atx_heading(line: &str, pedantic: bool) -> Option<(u8, &str)> {
    let (indent, line) = split_leading_ws(line);
    if (!pedantic && indent > 3) || (pedantic && indent > 0) {
        return None;
    }

    let mut count = 0usize;
    let mut i = 0usize;
    while i < line.len() {
        if line.as_bytes()[i] == b'#' {
            count += 1;
            if count > 6 {
                return None;
            }
            i += 1;
        } else {
            break;
        }
    }

    if count > 0 {
        let rest = &line[i..];
        if rest.is_empty() {
            return Some((count as u8, ""));
        }
        if !pedantic && !rest.starts_with(' ') && !rest.starts_with('\t') {
            return None;
        }

        let text = rest.trim_start_matches(|c: char| c == ' ' || c == '\t');
        let mut title_end = text.len();
        while title_end > 0 {
            let ch = text.as_bytes()[title_end - 1];
            if ch == b' ' || ch == b'\t' {
                title_end -= 1;
                continue;
            }
            break;
        }

        let mut closing_start = title_end;
        while closing_start > 0 {
            let ch = text.as_bytes()[closing_start - 1];
            if ch == b'#' {
                closing_start -= 1;
                continue;
            }
            break;
        }

        if closing_start < title_end {
            if pedantic || closing_start == 0 || {
                let ch = text.as_bytes()[closing_start - 1];
                ch == b' ' || ch == b'\t'
            } {
                title_end = closing_start;
            }
        }

        while title_end > 0 {
            let ch = text.as_bytes()[title_end - 1];
            if ch == b' ' || ch == b'\t' {
                title_end -= 1;
                continue;
            }
            break;
        }

        let text = &text[..title_end];
        return Some((count as u8, text));
    }
    None
}

fn parse_setext_heading(line: &str, underline: &str) -> Option<u8> {
    let (indent, line) = split_leading_ws(trim_setext_heading_line(line));
    if indent > 3 {
        return None;
    }
    let (underline_indent, underline) = split_leading_ws(underline);
    if underline_indent > 3 {
        return None;
    }

    if line.starts_with(LAZY_QUOTE_PREFIX) || underline.starts_with(LAZY_QUOTE_PREFIX) {
        return None;
    }
    if line.trim().is_empty() {
        return None;
    }
    if line_has_list_marker_with_content(line) {
        return None;
    }

    let trimmed = underline.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c == '=') {
        return Some(1);
    }
    if trimmed.chars().all(|c| c == '-') {
        return Some(2);
    }
    None
}

fn parse_setext_heading_block(
    lines: &[&str],
    start: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Option<(ast::Block, usize)> {
    if list_marker(strip_lazy_prefix(lines[start]).trim_start()).is_some() {
        return None;
    }

    let mut paragraph_lines = Vec::new();
    let mut i = start;

    while i < lines.len() {
        let line = strip_lazy_prefix(lines[i]);
        if line.trim().is_empty() {
            return None;
        }
        if parse_reference_definition(line, pedantic).is_some() {
            return None;
        }
        if parse_atx_heading(line, pedantic).is_some()
            || parse_thematic_break(line).is_some()
            || parse_fenced_code_block(lines, i).is_some()
            || parse_indented_code_block(lines, i).is_some()
            || (parse_list_block(lines, i, gfm, pedantic, refs).is_some()
                && list_interrupts_paragraph(line))
            || parse_blockquote_block(lines, i).is_some()
            || parse_table_block(lines, i, gfm, pedantic, refs).is_some()
            || parse_html_block(lines, i).is_some()
        {
            return None;
        }

        if pedantic && !paragraph_lines.is_empty() {
            return None;
        }

        paragraph_lines.push(trim_setext_heading_line(line));

        let Some(next_line) = lines.get(i + 1) else {
            return None;
        };
        if let Some(level) = parse_setext_heading(line, next_line) {
            let text = paragraph_lines.join("\n");
            let inlines = parse_block_inlines(&text, gfm, pedantic, refs);
            return Some((ast::Block::Heading { level, inlines }, i + 2));
        }

        i += 1;
    }

    None
}

fn parse_fenced_code_block(
    lines: &[&str],
    start: usize,
) -> Option<(bool, Option<String>, usize, bool, usize)> {
    let (indent, opening) = split_leading_ws(lines[start]);
    if indent > 3 {
        return None;
    }
    if opening.is_empty() {
        return None;
    }
    if !opening.starts_with('`') && !opening.starts_with('~') {
        return None;
    }

    let fence_char = opening.as_bytes()[0];
    let mut fence_len = 0usize;
    while fence_len < opening.len() && opening.as_bytes()[fence_len] == fence_char {
        fence_len += 1;
    }
    if fence_len < 3 {
        return None;
    }

    let raw_info = opening[fence_len..].trim();
    if fence_char == b'`' && raw_info.contains('`') {
        return None;
    }
    let info = decode_html_entities(raw_info);
    let info = (!info.is_empty()).then_some(info);

    let mut end = start + 1;
    while end < lines.len() {
        if is_fenced_code_close(lines[end], fence_char, fence_len) {
            return Some((true, info, end + 1, true, indent));
        }
        end += 1;
    }

    Some((true, info, lines.len(), false, indent))
}

fn parse_indented_code_block(lines: &[&str], start: usize) -> Option<(String, usize)> {
    if !is_indented_code_line(lines[start]) {
        return None;
    }

    let mut i = start;
    while i + 1 < lines.len() {
        if is_markdown_blank(lines[i + 1]) {
            i += 1;
            continue;
        }

        if is_indented_code_line(lines[i + 1]) {
            i += 1;
            continue;
        }

        break;
    }

    let mut raw = String::with_capacity(
        lines[start..=i]
            .iter()
            .map(|line| line.len())
            .sum::<usize>()
            + i.saturating_sub(start),
    );
    for (idx, line) in lines[start..=i].iter().copied().enumerate() {
        if idx > 0 {
            raw.push('\n');
        }
        raw.push_str(&strip_indentation(line));
    }
    raw.push('\n');
    Some((raw, i + 1))
}

fn is_indented_code_line(line: &str) -> bool {
    split_leading_ws(line).0 >= 4
}

fn strip_indentation(line: &str) -> String {
    if is_markdown_blank(line) {
        return String::new();
    }

    if let Some(rest) = line.strip_prefix('\t') {
        return rest.to_string();
    }

    let mut byte_idx = 0usize;
    while byte_idx < line.len() && byte_idx < 4 && line.as_bytes()[byte_idx] == b' ' {
        byte_idx += 1;
    }
    line[byte_idx..].to_string()
}

fn parse_list_block(
    lines: &[&str],
    start: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Option<(bool, usize, bool, Vec<ast::ListItem>, usize)> {
    let (base_indent, line) = split_leading_ws(lines[start]);
    let marker = match list_marker(line) {
        Some(v) => v,
        None => return None,
    };
    let ordered = marker.ordered;
    let start_value = marker.start;
    let marker_kind = marker.kind;

    let mut items = Vec::new();
    let mut tight = true;
    let mut i = start;
    while i < lines.len() {
        let (indent, text) = split_leading_ws(lines[i]);
        if let Some(next_marker) = list_marker(text) {
            if next_marker.ordered != ordered || (!pedantic && next_marker.kind != marker_kind) {
                break;
            }
            if !list_marker_indent_matches_level(base_indent, indent) {
                break;
            }
            if parse_thematic_break(text).is_some() {
                break;
            }
            let rest = text.get(next_marker.end..).unwrap_or("");
            let marker_cols = indent + next_marker.end;
            let (content, padding_cols) = strip_list_marker_padding_with_cols(rest, marker_cols);
            let content_indent = indent + next_marker.end + padding_cols.max(1);
            let has_initial_content = !content.is_empty();
            let starts_with_block = list_item_starts_with_block(content, pedantic);
            let (item_end, _) = collect_list_item_with_content_indent(
                lines,
                i,
                indent,
                base_indent,
                content_indent,
                has_initial_content,
                starts_with_block,
                pedantic,
            );
            let trailing_blank_lines = lines[i..item_end]
                .iter()
                .rev()
                .take_while(|line| line.trim().is_empty())
                .count();
            let content_end = item_end.saturating_sub(trailing_blank_lines);
            let blank_separates_same_list = trailing_blank_lines > 0
                && next_line_continues_same_list(
                    lines,
                    item_end,
                    indent,
                    ordered,
                    marker_kind,
                    pedantic,
                );
            let item = parse_list_item(
                &lines[i..item_end],
                next_marker.end,
                content_indent,
                gfm,
                pedantic,
                refs,
            );
            let paragraph_count = item
                .children
                .iter()
                .filter(|child| matches!(child, ast::Block::Paragraph { .. }))
                .count();
            let only_nested_lists_after_first = item.children.len() > 1
                && item
                    .children
                    .iter()
                    .skip(1)
                    .all(|child| matches!(child, ast::Block::List { .. }));
            let loose_from_blank_break = paragraph_count > 0
                && list_item_has_top_level_blank_break(
                    &lines[i..content_end],
                    content_indent,
                    pedantic,
                );
            let loose_from_blank_indented_code = paragraph_count > 0
                && list_item_has_top_level_blank_indented_code(
                    &lines[i..content_end],
                    content_indent,
                    pedantic,
                )
                && !only_nested_lists_after_first;
            let loose_from_item =
                paragraph_count > 1 || loose_from_blank_break || loose_from_blank_indented_code;
            tight &= !blank_separates_same_list && !loose_from_item;
            items.push(item);
            i = item_end;
            continue;
        }
        break;
    }

    if items.is_empty() {
        None
    } else {
        Some((ordered, start_value, tight, items, i))
    }
}

fn parse_list_item(
    lines: &[&str],
    _marker_end: usize,
    content_indent: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> ast::ListItem {
    let first = lines[0];
    let (first_indent, first_marker) = split_leading_ws(first);
    let marker_end = list_marker(first_marker)
        .map(|marker| marker.end)
        .unwrap_or(1);
    let first_content_raw =
        strip_single_padding_marker(&first_marker[marker_end..], first_indent + marker_end);
    let (first_content, task) = if gfm {
        parse_task_prefix(&first_content_raw)
    } else {
        (first_content_raw.as_str(), None)
    };
    let force_first_paragraph = task.is_some();

    let mut item_lines = Vec::new();
    item_lines.push(if force_first_paragraph {
        force_paragraph_line(first_content)
    } else {
        first_content.to_string()
    });
    for raw in lines.iter().skip(1) {
        let (raw_indent, _) = split_leading_ws(raw);
        let normalized = normalize_pedantic_list_line(raw, pedantic);
        let normalized = normalized.as_ref();
        let (indent, text) = split_leading_ws(normalized);
        let stripped = strip_leading_content_indent(normalized, content_indent);
        let pedantic_nested_list =
            pedantic && raw_indent > first_indent && list_marker(text).is_some();
        if indent < content_indent && !text.is_empty() && !pedantic_nested_list {
            item_lines.push(force_paragraph_line(&stripped));
        } else {
            item_lines.push(stripped);
        }
    }

    if !force_first_paragraph {
        if let Some((id, def)) = parse_reference_definition(first_content, pedantic) {
            if item_lines.iter().skip(1).all(|line| line.trim().is_empty()) {
                refs.entry(id).or_insert(def);
                item_lines[0].clear();
            }
        }
    }

    let child_line_len = if item_lines.last().is_some_and(|line| line.is_empty()) {
        item_lines.len().saturating_sub(1)
    } else {
        item_lines.len()
    };
    let child_line_refs = item_lines[..child_line_len]
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let children = match parse_blocks_from_lines(&child_line_refs, gfm, pedantic, refs) {
        ast::Document::Nodes(nodes) => nodes,
    };

    ast::ListItem { children, task }
}

fn collect_list_item_with_content_indent(
    lines: &[&str],
    start: usize,
    item_indent: usize,
    list_base_indent: usize,
    content_indent: usize,
    has_initial_content: bool,
    starts_with_block: bool,
    pedantic: bool,
) -> (usize, bool) {
    let mut i = start;
    let mut end = start + 1;
    let mut had_blank = false;
    let mut saw_blank = false;
    let mut saw_item_content = has_initial_content;

    while i + 1 < lines.len() {
        let next = lines[i + 1];
        if next.trim().is_empty() {
            i += 1;
            end = i + 1;
            had_blank = true;
            saw_blank = true;
            continue;
        }

        let normalized_next = normalize_pedantic_list_line(next, pedantic);
        let normalized_next = normalized_next.as_ref();
        let (raw_indent, _) = split_leading_ws(next);
        let (indent, text) = split_leading_ws(normalized_next);
        if !has_initial_content && indent <= item_indent {
            break;
        }
        if starts_with_block && indent <= item_indent {
            break;
        }

        if had_blank && indent <= item_indent && text.trim_start().starts_with('>') {
            if stripped_blockquote_content_indent(text) < 4 {
                break;
            }
        }
        if had_blank
            && indent <= item_indent
            && parse_reference_definition(text, pedantic).is_some()
        {
            break;
        }
        if had_blank && !saw_item_content && indent <= content_indent {
            break;
        }
        if had_blank
            && indent < content_indent
            && !(pedantic && list_marker(text).is_some() && raw_indent > item_indent)
            && !(pedantic
                && indent >= item_indent
                && item_indent > 0
                && item_indent == list_base_indent
                && list_marker(text).is_none()
                && !is_block_boundary_without_quote(Some(text), pedantic))
            && !(text.trim_start().starts_with('>')
                && stripped_blockquote_content_indent(text) >= 4)
        {
            break;
        }

        if indent <= item_indent
            && is_block_boundary_without_quote(Some(text), pedantic)
            && !(pedantic && list_marker(text).is_some() && raw_indent > item_indent)
            && !(text.trim_start().starts_with('>')
                && stripped_blockquote_content_indent(text) >= 4)
        {
            break;
        }

        if indent < content_indent
            && list_marker(text).is_some()
            && list_base_indent == 0
            && list_marker_indent_matches_level(list_base_indent, indent)
            && !(pedantic && raw_indent > item_indent)
        {
            break;
        }

        if indent < content_indent && text.trim_start().starts_with('>') {
            break;
        }

        if content_indent > 0 && indent < content_indent && is_fenced_code_start(next) {
            break;
        }

        i += 1;
        end = i + 1;
        had_blank = false;
        saw_item_content = saw_item_content || !text.is_empty();
    }

    (end.min(lines.len()), saw_blank)
}

fn normalize_pedantic_list_line(line: &str, pedantic: bool) -> Cow<'_, str> {
    if pedantic {
        Cow::Owned(normalize_pedantic_list_nesting(line))
    } else {
        Cow::Borrowed(line)
    }
}

fn normalize_pedantic_list_nesting(line: &str) -> String {
    let space_count = line.bytes().take_while(|b| *b == b' ').count();
    if space_count == 0 || space_count == line.len() {
        return line.to_string();
    }
    if line.as_bytes().get(space_count).copied() == Some(b'\t') {
        return line.to_string();
    }

    let replaced = match space_count % 4 {
        0 => 4,
        rem => rem,
    };
    let normalized_spaces = space_count - replaced + 2;
    let mut out = String::with_capacity(line.len() + 1);
    out.push_str(&" ".repeat(normalized_spaces));
    out.push_str(&line[space_count..]);
    out
}

fn strip_list_marker_padding_with_cols(rest: &str, marker_cols: usize) -> (&str, usize) {
    let space_count = rest.bytes().take_while(|b| *b == b' ').count();
    if space_count > 4 {
        return (&rest[1..], 1);
    }

    let mut byte_idx = 0usize;
    let mut cols = marker_cols;
    while byte_idx < rest.len() {
        match rest.as_bytes()[byte_idx] {
            b' ' => {
                byte_idx += 1;
                cols += 1;
            }
            b'\t' => {
                byte_idx += 1;
                cols += 4 - (cols % 4);
                break;
            }
            _ => break,
        }
    }

    if byte_idx == 0 {
        return (rest, 0);
    }

    (&rest[byte_idx..], cols.saturating_sub(marker_cols))
}

fn list_item_starts_with_block(content: &str, pedantic: bool) -> bool {
    parse_thematic_break(content).is_some()
        || parse_atx_heading(content, pedantic).is_some()
        || is_fenced_code_start(content)
        || is_indented_code_line(content)
}

fn next_line_continues_same_list(
    lines: &[&str],
    next_idx: usize,
    base_indent: usize,
    ordered: bool,
    marker_kind: u8,
    pedantic: bool,
) -> bool {
    let Some(line) = lines.get(next_idx) else {
        return false;
    };
    let (indent, text) = split_leading_ws(line);
    if !list_marker_indent_matches_level(base_indent, indent) {
        return false;
    }
    matches!(
        list_marker(text),
        Some(next_marker)
            if next_marker.ordered == ordered
                && (pedantic || next_marker.kind == marker_kind)
    )
}

fn list_item_has_top_level_blank_break(
    lines: &[&str],
    content_indent: usize,
    pedantic: bool,
) -> bool {
    let mut saw_blank = false;

    for raw in lines.iter().skip(1) {
        if raw.trim().is_empty() {
            saw_blank = true;
            continue;
        }
        if !saw_blank {
            continue;
        }

        let normalized = normalize_pedantic_list_line(raw, pedantic);
        let dedented = strip_leading_content_indent(normalized.as_ref(), content_indent);
        let (indent, text) = split_leading_ws(&dedented);
        if parse_reference_definition(text, pedantic).is_some()
            || list_marker(text).is_some()
            || text.trim_start().starts_with('>')
            || parse_thematic_break(text).is_some()
            || parse_atx_heading(text, pedantic).is_some()
            || is_fenced_code_start(text)
            || html_block_interrupts_paragraph(&[text], 0)
            || indent == 0
        {
            return true;
        }

        saw_blank = false;
    }

    false
}

fn list_item_has_top_level_blank_indented_code(
    lines: &[&str],
    content_indent: usize,
    pedantic: bool,
) -> bool {
    let mut saw_blank = false;

    for raw in lines.iter().skip(1) {
        if raw.trim().is_empty() {
            saw_blank = true;
            continue;
        }
        if !saw_blank {
            continue;
        }

        let normalized = normalize_pedantic_list_line(raw, pedantic);
        let dedented = strip_leading_content_indent(normalized.as_ref(), content_indent);
        if is_indented_code_line(&dedented) {
            return true;
        }

        saw_blank = false;
    }

    false
}

fn list_marker_indent_matches_level(base_indent: usize, indent: usize) -> bool {
    if base_indent <= 3 {
        indent <= 3
    } else {
        indent >= base_indent && indent <= base_indent + 3
    }
}

fn stripped_blockquote_content_indent(line: &str) -> usize {
    let mut rest = line.trim_start();
    let mut saw_quote = false;

    while let Some(next) = rest.strip_prefix('>') {
        saw_quote = true;
        rest = next.strip_prefix(' ').unwrap_or(next);
    }

    if !saw_quote {
        return 0;
    }

    split_leading_ws(rest).0
}

fn strip_leading_content_indent(line: &str, content_indent: usize) -> String {
    if line.is_empty() {
        return String::new();
    }

    let (indent_cols, tail) = split_leading_ws(line);
    if indent_cols == 0 {
        return line.to_string();
    }

    let residual_cols = indent_cols.saturating_sub(content_indent);
    let mut out = String::with_capacity(residual_cols + tail.len());
    out.push_str(&" ".repeat(residual_cols));
    out.push_str(tail);
    out
}

fn strip_code_fence_indent(line: &str, indent: usize) -> String {
    if indent == 0 || line.is_empty() {
        return line.to_string();
    }

    let mut byte_idx = 0usize;
    let mut removed_cols = 0usize;

    while byte_idx < line.len() && removed_cols < indent {
        match line.as_bytes()[byte_idx] {
            b' ' => {
                byte_idx += 1;
                removed_cols += 1;
            }
            b'\t' => {
                byte_idx += 1;
                removed_cols += 4 - (removed_cols % 4);
            }
            _ => break,
        }
    }

    if removed_cols < indent {
        return line.to_string();
    }

    line[byte_idx..].to_string()
}

fn is_fenced_code_start(line: &str) -> bool {
    parse_fence_run(line).is_some()
}

fn parse_fence_run(line: &str) -> Option<(u8, usize)> {
    let (indent, line) = split_leading_ws(line);
    if indent > 3 {
        return None;
    }
    let trimmed = line.trim_start();
    let bytes = trimmed.as_bytes();
    let first = *bytes.first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let mut len = 0usize;
    while len < bytes.len() && bytes[len] == first {
        len += 1;
    }
    if len < 3 {
        return None;
    }
    Some((first, len))
}

fn is_fenced_code_close(line: &str, fence_char: u8, fence_len: usize) -> bool {
    let (indent, line) = split_leading_ws(line);
    if indent > 3 {
        return false;
    }

    let bytes = line.as_bytes();
    if bytes.is_empty() || bytes[0] != fence_char {
        return false;
    }

    let mut idx = 0usize;
    while idx < bytes.len() && bytes[idx] == fence_char {
        idx += 1;
    }

    if idx < fence_len {
        return false;
    }

    while idx < bytes.len() {
        if !matches!(bytes[idx], b' ' | b'\t') {
            return false;
        }
        idx += 1;
    }

    true
}

#[inline]
fn split_leading_ws(line: &str) -> (usize, &str) {
    let mut byte_idx = 0usize;
    let mut cols = 0usize;

    while byte_idx < line.len() {
        match line.as_bytes()[byte_idx] {
            b' ' => {
                byte_idx += 1;
                cols += 1;
            }
            b'\t' => {
                byte_idx += 1;
                cols += 4 - (cols % 4);
            }
            _ => break,
        }
    }

    (cols, &line[byte_idx..])
}

fn split_leading_ws_from_column(line: &str, start_col: usize) -> (usize, &str) {
    let mut byte_idx = 0usize;
    let mut cols = start_col;

    while byte_idx < line.len() {
        match line.as_bytes()[byte_idx] {
            b' ' => {
                byte_idx += 1;
                cols += 1;
            }
            b'\t' => {
                byte_idx += 1;
                cols += 4 - (cols % 4);
            }
            _ => break,
        }
    }

    (cols.saturating_sub(start_col), &line[byte_idx..])
}

fn strip_single_padding_marker(rest: &str, start_col: usize) -> String {
    if !matches!(rest.as_bytes().first().copied(), Some(b' ' | b'\t')) {
        return rest.to_string();
    }

    let (indent_cols, tail) = split_leading_ws_from_column(rest, start_col);
    let residual_cols = indent_cols.saturating_sub(1);
    let mut out = String::with_capacity(residual_cols + tail.len());
    out.push_str(&" ".repeat(residual_cols));
    out.push_str(tail);
    out
}

fn strip_blockquote_padding_marker(rest: &str) -> String {
    match rest.as_bytes().first().copied() {
        Some(b' ' | b'\t') => rest[1..].to_string(),
        _ => rest.to_string(),
    }
}

fn parse_task_prefix(text: &str) -> (&str, Option<bool>) {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix("[ ]") {
        let content = rest.trim_start();
        if content.is_empty() {
            return (trimmed.trim_end(), None);
        }
        return (content, Some(false));
    }
    if let Some(rest) = trimmed.strip_prefix("[x]") {
        let content = rest.trim_start();
        if content.is_empty() {
            return (trimmed.trim_end(), None);
        }
        return (content, Some(true));
    }
    if let Some(rest) = trimmed.strip_prefix("[X]") {
        let content = rest.trim_start();
        if content.is_empty() {
            return (trimmed.trim_end(), None);
        }
        return (content, Some(true));
    }
    (text, None)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ListMarker {
    ordered: bool,
    kind: u8,
    end: usize,
    start: usize,
}

fn list_marker(line: &str) -> Option<ListMarker> {
    let b = line.as_bytes();
    if !b.is_empty() && (b[0] == b'-' || b[0] == b'+' || b[0] == b'*') {
        if b.len() == 1 || b[1].is_ascii_whitespace() {
            return Some(ListMarker {
                ordered: false,
                kind: b[0],
                end: 1,
                start: 1,
            });
        }
    }

    if b.is_empty() || !b[0].is_ascii_digit() {
        return None;
    }

    let mut pos = 0usize;
    while pos < b.len() && b[pos].is_ascii_digit() {
        pos += 1;
        if pos > 9 {
            return None;
        }
    }
    if pos == b.len() {
        return None;
    }

    let Some(sep) = b.get(pos) else {
        return None;
    };
    if *sep != b'.' && *sep != b')' {
        return None;
    }
    let start = std::str::from_utf8(&b[..pos]).ok()?.parse::<usize>().ok()?;
    let after = &line[pos + 1..];
    if after.is_empty() || after.starts_with(' ') || after.starts_with('\t') {
        return Some(ListMarker {
            ordered: true,
            kind: *sep,
            end: pos + 1,
            start,
        });
    }

    None
}

fn parse_blockquote_block(lines: &[&str], start: usize) -> Option<(usize, Vec<String>, usize)> {
    let (indent, line) = split_leading_ws(strip_lazy_prefix(lines[start]));
    if indent > 3 {
        return None;
    }
    if !line.starts_with('>') {
        return None;
    }

    let mut i = start;
    let mut out = Vec::new();
    let mut can_lazy_continue = false;
    let mut open_fence: Option<(u8, usize)> = None;

    while i < lines.len() {
        let line = strip_lazy_prefix(lines[i]);
        let (inner_indent, raw) = split_leading_ws(line);
        if inner_indent > 3 {
            if out.is_empty() || open_fence.is_some() || !can_lazy_continue {
                break;
            }
            out.push(format!("{}{}", LAZY_QUOTE_PREFIX, raw));
            i += 1;
            continue;
        }

        if let Some(rest) = raw.strip_prefix('>') {
            let rest = strip_blockquote_padding_marker(rest);
            if let Some((fence_char, fence_len)) = open_fence {
                if is_fenced_code_close(&rest, fence_char, fence_len) {
                    open_fence = None;
                }
            } else if let Some((fence_char, fence_len)) = parse_fence_run(&rest) {
                open_fence = Some((fence_char, fence_len));
            }
            can_lazy_continue = open_fence.is_none()
                && !rest.is_empty()
                && !is_indented_code_line(&rest)
                && (!is_block_boundary_without_quote(Some(&rest), false)
                    || blockquote_content_allows_lazy_continuation(&rest));
            out.push(rest);
            i += 1;
            continue;
        }

        if raw.is_empty() {
            if out.is_empty() {
                return None;
            }
            break;
        }

        if out.is_empty() {
            return None;
        }
        if open_fence.is_some() || !can_lazy_continue {
            break;
        }
        if is_blockquote_boundary(
            Some(raw),
            lines
                .get(i + 1)
                .map(|line| strip_lazy_prefix(line).trim_start()),
        ) {
            break;
        }

        out.push(format!("{}{}", LAZY_QUOTE_PREFIX, raw));
        i += 1;
    }

    Some((0, out, i))
}

fn blockquote_content_allows_lazy_continuation(rest: &str) -> bool {
    let (_, text) = split_leading_ws(rest);
    let Some(marker) = list_marker(text) else {
        return false;
    };
    let after_marker = text.get(marker.end..).unwrap_or("");
    let (content, _) = strip_list_marker_padding_with_cols(after_marker, marker.end);
    if content.is_empty() {
        return false;
    }

    if let Some(nested_quote) = content.strip_prefix('>') {
        let nested_quote = strip_blockquote_padding_marker(nested_quote);
        return !nested_quote.is_empty()
            && !is_indented_code_line(&nested_quote)
            && !is_block_boundary_without_quote(Some(&nested_quote), false);
    }

    !list_item_starts_with_block(content, false)
}

fn is_blockquote_boundary(current: Option<&str>, next: Option<&str>) -> bool {
    let Some(current) = current else {
        return false;
    };
    let trimmed = current.trim();
    if trimmed.is_empty() {
        return false;
    }

    // Keep short setext-style underline markers inside quoted lazy paragraphs,
    // but let thematic breaks terminate the quote when they are not quoted.
    if is_setext_underline(trimmed) && parse_thematic_break(trimmed).is_none() {
        return false;
    }

    if is_unindented_table_row_start(current, next) {
        return true;
    }

    is_block_boundary_without_quote(Some(trimmed), false)
}

fn is_unindented_table_row_start(current: &str, next: Option<&str>) -> bool {
    if current.trim().is_empty() {
        return false;
    }
    if current.trim().starts_with('|') {
        return false;
    }
    if !current.contains('|') {
        return false;
    }

    matches!(
        next,
        Some(next) if parse_table_delimiter(next).is_some()
    )
}

fn is_html_tag_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-'
}

fn is_html_attribute_name_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || matches!(ch, '_' | ':')
}

fn is_html_attribute_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.' | '-')
}

fn parse_html_open_tag(line: &str) -> Option<(&str, usize)> {
    let bytes = line.as_bytes();
    if bytes.first().copied() != Some(b'<') {
        return None;
    }

    let mut i = 1usize;
    let first = *bytes.get(i)? as char;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    i += 1;
    while i < bytes.len() && is_html_tag_name_char(bytes[i] as char) {
        i += 1;
    }
    let tag_name = &line[1..i];

    loop {
        let mut had_space = false;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
            i += 1;
            had_space = true;
        }
        if i >= bytes.len() {
            return None;
        }

        match bytes[i] {
            b'>' => return Some((tag_name, i + 1)),
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'>' => return Some((tag_name, i + 2)),
            _ => {}
        }

        if !had_space {
            return None;
        }

        let first = bytes[i] as char;
        if !is_html_attribute_name_start(first) {
            return None;
        }
        i += 1;
        while i < bytes.len() && is_html_attribute_name_char(bytes[i] as char) {
            i += 1;
        }

        let attr_end = i;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        if bytes[i] != b'=' {
            i = attr_end;
            continue;
        }

        i += 1;
        while i < bytes.len() && matches!(bytes[i], b' ' | b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }

        match bytes[i] {
            b'\'' | b'"' => {
                let quote = bytes[i];
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                if i >= bytes.len() {
                    return None;
                }
                i += 1;
            }
            b' ' | b'\t' | b'>' => return None,
            _ => {
                while i < bytes.len() {
                    match bytes[i] {
                        b' ' | b'\t' | b'>' => break,
                        b'"' | b'\'' | b'=' | b'<' | b'`' => return None,
                        _ => i += 1,
                    }
                }
            }
        }
    }
}

fn parse_html_tag_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let raw = trimmed.strip_prefix('<')?;
    let first = raw.chars().next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let name_end = raw
        .find(|c: char| !is_html_tag_name_char(c))
        .unwrap_or(raw.len());
    let next = raw[name_end..].chars().next();
    if next.is_some_and(|ch| !matches!(ch, ' ' | '\t' | '>' | '/')) {
        return None;
    }
    Some(&raw[..name_end])
}

fn parse_html_closing_tag(line: &str) -> Option<(&str, usize)> {
    let trimmed = line.trim_start();
    let raw = trimmed.strip_prefix("</")?;
    if raw.is_empty() {
        return None;
    }

    let mut chars = raw.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    let mut name_end = first.len_utf8();
    for (idx, ch) in chars {
        if !is_html_tag_name_char(ch) {
            break;
        }
        name_end = idx + ch.len_utf8();
    }

    let tag_name = &raw[..name_end];
    let rest = raw[name_end..].trim_start();
    if !rest.starts_with('>') {
        return None;
    }

    let consumed = trimmed.len() - rest.len() + 1;
    Some((tag_name, consumed))
}

fn parse_closing_html_tag_name(line: &str) -> Option<&str> {
    parse_html_closing_tag(line).map(|(tag_name, _)| tag_name)
}

fn is_complete_html_tag_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if let Some((_, consumed)) = parse_html_open_tag(trimmed) {
        return consumed == trimmed.len();
    }
    if let Some((_, consumed)) = parse_html_closing_tag(trimmed) {
        return consumed == trimmed.len();
    }
    false
}

fn consume_html_block_until_blank(lines: &[&str], start: usize) -> (ast::Block, usize) {
    let mut end = start + 1;
    while end < lines.len() && !lines[end].trim().is_empty() {
        end += 1;
    }
    (
        ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
        end,
    )
}

fn is_void_html_tag(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "area"
            | "base"
            | "basefont"
            | "bgsound"
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

fn is_block_html_tag(tag_name: &str) -> bool {
    tag_name.eq_ignore_ascii_case("address")
        || tag_name.eq_ignore_ascii_case("article")
        || tag_name.eq_ignore_ascii_case("aside")
        || tag_name.eq_ignore_ascii_case("base")
        || tag_name.eq_ignore_ascii_case("basefont")
        || tag_name.eq_ignore_ascii_case("blockquote")
        || tag_name.eq_ignore_ascii_case("body")
        || tag_name.eq_ignore_ascii_case("caption")
        || tag_name.eq_ignore_ascii_case("center")
        || tag_name.eq_ignore_ascii_case("col")
        || tag_name.eq_ignore_ascii_case("colgroup")
        || tag_name.eq_ignore_ascii_case("dd")
        || tag_name.eq_ignore_ascii_case("details")
        || tag_name.eq_ignore_ascii_case("dialog")
        || tag_name.eq_ignore_ascii_case("dir")
        || tag_name.eq_ignore_ascii_case("div")
        || tag_name.eq_ignore_ascii_case("dl")
        || tag_name.eq_ignore_ascii_case("dt")
        || tag_name.eq_ignore_ascii_case("fieldset")
        || tag_name.eq_ignore_ascii_case("figcaption")
        || tag_name.eq_ignore_ascii_case("figure")
        || tag_name.eq_ignore_ascii_case("footer")
        || tag_name.eq_ignore_ascii_case("form")
        || tag_name.eq_ignore_ascii_case("frame")
        || tag_name.eq_ignore_ascii_case("frameset")
        || tag_name.eq_ignore_ascii_case("h1")
        || tag_name.eq_ignore_ascii_case("h2")
        || tag_name.eq_ignore_ascii_case("h3")
        || tag_name.eq_ignore_ascii_case("h4")
        || tag_name.eq_ignore_ascii_case("h5")
        || tag_name.eq_ignore_ascii_case("h6")
        || tag_name.eq_ignore_ascii_case("head")
        || tag_name.eq_ignore_ascii_case("header")
        || tag_name.eq_ignore_ascii_case("hr")
        || tag_name.eq_ignore_ascii_case("html")
        || tag_name.eq_ignore_ascii_case("iframe")
        || tag_name.eq_ignore_ascii_case("legend")
        || tag_name.eq_ignore_ascii_case("li")
        || tag_name.eq_ignore_ascii_case("link")
        || tag_name.eq_ignore_ascii_case("main")
        || tag_name.eq_ignore_ascii_case("menu")
        || tag_name.eq_ignore_ascii_case("menuitem")
        || tag_name.eq_ignore_ascii_case("nav")
        || tag_name.eq_ignore_ascii_case("noframes")
        || tag_name.eq_ignore_ascii_case("ol")
        || tag_name.eq_ignore_ascii_case("optgroup")
        || tag_name.eq_ignore_ascii_case("option")
        || tag_name.eq_ignore_ascii_case("p")
        || tag_name.eq_ignore_ascii_case("param")
        || tag_name.eq_ignore_ascii_case("search")
        || tag_name.eq_ignore_ascii_case("section")
        || tag_name.eq_ignore_ascii_case("summary")
        || tag_name.eq_ignore_ascii_case("table")
        || tag_name.eq_ignore_ascii_case("tbody")
        || tag_name.eq_ignore_ascii_case("td")
        || tag_name.eq_ignore_ascii_case("tfoot")
        || tag_name.eq_ignore_ascii_case("th")
        || tag_name.eq_ignore_ascii_case("thead")
        || tag_name.eq_ignore_ascii_case("title")
        || tag_name.eq_ignore_ascii_case("tr")
        || tag_name.eq_ignore_ascii_case("track")
        || tag_name.eq_ignore_ascii_case("ul")
}

fn parse_html_block(lines: &[&str], start: usize) -> Option<(ast::Block, usize)> {
    let (indent, line) = split_leading_ws(lines[start]);
    if indent > 3 {
        return None;
    }
    let line = line.trim_start();
    if !line.starts_with('<') {
        return None;
    }

    if line.starts_with("<!--") {
        let mut end = start + 1;
        if line.contains("-->") {
            let mut raw = expand_tabs_html(lines[start]);
            if raw.starts_with("<!-->") || raw.starts_with("<!--->") {
                if let Some(prefix) = raw.strip_suffix("-->") {
                    raw = format!("{prefix}--&gt;");
                }
            }
            return Some((ast::Block::HtmlBlock(raw), end));
        }
        while end < lines.len() && !lines[end].contains("-->") {
            end += 1;
        }
        if end < lines.len() {
            end += 1;
        }
        return Some((
            ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
            end,
        ));
    }

    if line.starts_with("<?") {
        let mut end = start + 1;
        if line.contains("?>") {
            return Some((ast::Block::HtmlBlock(expand_tabs_html(lines[start])), end));
        }
        while end < lines.len() && !lines[end].contains("?>") {
            end += 1;
        }
        if end < lines.len() {
            end += 1;
        }
        return Some((
            ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
            end,
        ));
    }

    if line.starts_with("<![CDATA[") {
        let mut end = start + 1;
        if line.contains("]]>") {
            return Some((ast::Block::HtmlBlock(expand_tabs_html(lines[start])), end));
        }
        while end < lines.len() && !lines[end].contains("]]>") {
            end += 1;
        }
        if end < lines.len() {
            end += 1;
        }
        return Some((
            ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
            end,
        ));
    }

    if line.starts_with("<!")
        && line
            .as_bytes()
            .get(2)
            .is_some_and(|b| b.is_ascii_uppercase())
    {
        let mut end = start + 1;
        if line.contains('>') {
            return Some((ast::Block::HtmlBlock(expand_tabs_html(lines[start])), end));
        }
        while end < lines.len() && !lines[end].contains('>') {
            end += 1;
        }
        if end < lines.len() {
            end += 1;
        }
        return Some((
            ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
            end,
        ));
    }

    if let Some(tag_name) = parse_closing_html_tag_name(line) {
        if is_block_html_tag(tag_name) || is_complete_html_tag_line(line) {
            return Some(consume_html_block_until_blank(lines, start));
        }
        return None;
    }

    let Some(tag_name) = parse_html_tag_name(line) else {
        return None;
    };
    if matches!(tag_name, "script" | "pre" | "style" | "textarea") {
        let close = format!("</{tag_name}>");
        if line.contains(&close) {
            return Some((
                ast::Block::HtmlBlock(expand_tabs_html(lines[start])),
                start + 1,
            ));
        }

        let mut end = start + 1;
        while end < lines.len() {
            if lines[end].contains(&close) {
                end += 1;
                return Some((
                    ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
                    end,
                ));
            }
            end += 1;
        }

        return Some((
            ast::Block::HtmlBlock(join_html_block_lines(&lines[start..end])),
            end,
        ));
    }

    if is_block_html_tag(tag_name) {
        return Some(consume_html_block_until_blank(lines, start));
    }

    if is_void_html_tag(tag_name) || is_complete_html_tag_line(line) {
        return Some(consume_html_block_until_blank(lines, start));
    }

    None
}

fn expand_tabs_html(line: &str) -> String {
    line.to_string()
}

fn join_html_block_lines(lines: &[&str]) -> String {
    let mut out = String::with_capacity(
        lines.iter().map(|line| line.len()).sum::<usize>()
            + lines.len().saturating_sub(1),
    );
    for (idx, line) in lines.iter().copied().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

fn is_block_boundary_without_quote(next_line: Option<&str>, pedantic: bool) -> bool {
    let Some(line) = next_line else {
        return false;
    };
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.starts_with('|') {
        return true;
    }
    if parse_atx_heading(trimmed, pedantic).is_some() {
        return true;
    }
    if list_marker(trimmed).is_some() {
        return true;
    }
    if is_setext_underline(trimmed) {
        return true;
    }
    if is_thematic_break(trimmed) {
        return true;
    }
    if is_fenced_code_start(line) {
        return true;
    }
    if trimmed.starts_with('<') {
        return true;
    }
    false
}

fn is_setext_underline(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 2 {
        return false;
    }
    trimmed.chars().all(|c| c == '=' || c == '-')
}

fn is_thematic_break(line: &str) -> bool {
    parse_thematic_break(line).is_some()
}

fn parse_table_block(
    lines: &[&str],
    start: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Option<(ast::Block, usize)> {
    if !gfm {
        return None;
    }

    let delimiter = lines.get(start + 1)?;
    let aligns = parse_table_delimiter(delimiter)?;
    let header = parse_table_header(lines[start], aligns.len(), gfm, pedantic, refs)?;
    if header.is_empty() {
        return None;
    }
    let mut rows = Vec::new();
    let mut i = start + 2;
    let mut saw_implicit_tail_row = false;
    while i < lines.len() {
        if is_table_row(lines[i], aligns.len()) {
            if let Some(row) = parse_table_row(lines[i], aligns.len(), gfm, pedantic, refs) {
                saw_implicit_tail_row = !line_has_table_pipe(lines[i]);
                rows.push(row);
            }
            i += 1;
            continue;
        }

        if let Some(row) = parse_table_tail_row(
            lines,
            i,
            aligns.len(),
            saw_implicit_tail_row,
            gfm,
            pedantic,
            refs,
        ) {
            rows.push(row);
            saw_implicit_tail_row = true;
            i += 1;
            continue;
        }

        break;
    }

    Some((
        ast::Block::Table {
            aligns,
            header,
            rows,
        },
        i,
    ))
}

fn is_table_row(line: &str, columns: usize) -> bool {
    let (indent, text) = split_leading_ws(line);
    if indent > 3 || is_markdown_blank(text) {
        return false;
    }

    if text.contains('|') {
        return true;
    }

    columns == 1 && !text.contains('\n')
}

fn line_has_table_pipe(line: &str) -> bool {
    let (indent, text) = split_leading_ws(line);
    indent <= 3 && text.contains('|') && !is_markdown_blank(text)
}

fn parse_table_delimiter(line: &str) -> Option<Vec<Option<ast::TableAlignment>>> {
    let (indent, text) = split_leading_ws(line);
    if indent > 3 {
        return None;
    }
    let trimmed = text.trim();
    let has_pipe = trimmed.contains('|');
    let cols = split_table_cells(trimmed);
    if cols.is_empty() {
        return None;
    }

    if !cols.iter().all(|col| {
        let t = col.trim();
        !t.is_empty() && t.chars().all(|c| c == '-' || c == ':')
    }) {
        return None;
    }

    if !cols.iter().any(|col| col.contains('-')) {
        return None;
    }
    if !has_pipe && !trimmed.contains(':') {
        return None;
    }

    Some(
        cols.iter()
            .map(|col| {
                let trimmed = col.trim();
                let left = trimmed.starts_with(':');
                let right = trimmed.ends_with(':');
                match (left, right) {
                    (true, true) => Some(ast::TableAlignment::Center),
                    (true, false) => Some(ast::TableAlignment::Left),
                    (false, true) => Some(ast::TableAlignment::Right),
                    (false, false) => None,
                }
            })
            .collect(),
    )
}

fn parse_table_header(
    line: &str,
    columns: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Option<Vec<Vec<Inline>>> {
    let (indent, text) = split_leading_ws(line);
    if indent > 3 {
        return None;
    }

    if !text.contains('|') {
        if columns != 1 {
            return None;
        }
        return Some(vec![parse_block_inlines(text, gfm, pedantic, refs)]);
    }

    let parts = split_table_cells(text);
    if parts.is_empty() || parts.len() != columns {
        return None;
    }

    Some(parse_table_parts(&parts, columns, gfm, pedantic, refs))
}

fn parse_table_row(
    line: &str,
    columns: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Option<Vec<Vec<Inline>>> {
    let (indent, text) = split_leading_ws(line);
    if indent > 3 {
        return None;
    }

    if !text.contains('|') {
        if columns != 1 {
            return None;
        }
        return Some(vec![parse_block_inlines(text, gfm, pedantic, refs)]);
    }

    let parts = split_table_cells(text);
    if parts.is_empty() {
        return None;
    }

    Some(parse_table_parts(&parts, columns, gfm, pedantic, refs))
}

fn parse_table_parts(
    parts: &[String],
    columns: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Vec<Vec<Inline>> {
    let mut row = parts
        .iter()
        .map(|cell| {
            let normalized = unescape_table_cell(cell);
            parse_block_inlines(normalized.as_ref().trim(), gfm, pedantic, refs)
        })
        .collect::<Vec<_>>();
    while row.len() < columns {
        row.push(Vec::new());
    }
    row.truncate(columns);
    row
}

fn unescape_table_cell(cell: &str) -> Cow<'_, str> {
    if !cell.contains(r"\|") {
        return Cow::Borrowed(cell);
    }
    Cow::Owned(cell.replace(r"\|", "|"))
}

fn parse_table_tail_row(
    lines: &[&str],
    index: usize,
    columns: usize,
    saw_implicit_tail_row: bool,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Option<Vec<Vec<Inline>>> {
    if columns <= 1 {
        return None;
    }

    let line = *lines.get(index)?;
    let (indent, text) = split_leading_ws(line);
    if indent > 3 || is_markdown_blank(text) || text.contains('|') {
        return None;
    }

    if parse_atx_heading(text, pedantic).is_some()
        || parse_blockquote_block(lines, index).is_some()
        || parse_fenced_code_block(lines, index).is_some()
        || parse_indented_code_block(lines, index).is_some()
        || line_has_list_marker_with_content(text)
        || html_block_interrupts_paragraph(lines, index)
        || parse_reference_definition(text, pedantic).is_some()
    {
        return None;
    }

    let trimmed = text.trim();
    if !saw_implicit_tail_row
        && (parse_thematic_break(text).is_some() || is_setext_underline(trimmed))
    {
        return None;
    }

    let mut row = vec![parse_block_inlines(trimmed, gfm, pedantic, refs)];
    while row.len() < columns {
        row.push(Vec::new());
    }
    Some(row)
}

#[inline]
fn is_markdown_blank(line: &str) -> bool {
    line.chars().all(|ch| matches!(ch, ' ' | '\t'))
}

fn parse_reference_definition(line: &str, pedantic: bool) -> Option<(String, ReferenceDefinition)> {
    parse_reference_definition_full(line, pedantic)
}

fn parse_reference_definition_with_continuation(
    lines: &[&str],
    start: usize,
    pedantic: bool,
) -> Option<(String, ReferenceDefinition, usize)> {
    if start >= lines.len() {
        return None;
    }

    let mut best = None;
    let mut end = start;
    while end < lines.len() {
        if end > start && lines[end].trim().is_empty() {
            break;
        }

        // A following standalone definition starts a new entry; do not
        // let pedantic title continuation swallow it into the current one.
        if end > start && parse_reference_definition(lines[end], pedantic).is_some() {
            break;
        }

        let candidate = join_reference_definition_lines(lines, start, end);
        if let Some((id, def)) = parse_reference_definition_full(&candidate, pedantic) {
            best = Some((id, def, end - start + 1));
        }

        end += 1;
    }
    best
}

fn parse_reference_definition_full(
    raw: &str,
    pedantic: bool,
) -> Option<(String, ReferenceDefinition)> {
    if raw.contains("\n\n") || raw.contains("\r\n\r\n") {
        return None;
    }

    let (indent, trimmed_start) = split_leading_ws(raw);
    if indent > 3 || !trimmed_start.starts_with('[') {
        return None;
    }

    let close = trimmed_start.find("]:")?;
    let label = trimmed_start[1..close].trim();
    let key = try_normalize_reference_label(label)?;

    let rest = skip_reference_whitespace(&trimmed_start[close + 2..])?;
    if rest.is_empty() {
        return None;
    }

    let (href, remaining) = parse_reference_destination(rest)?;
    let trimmed_remaining = skip_reference_whitespace(remaining).unwrap_or(remaining);
    let had_separator = trimmed_remaining.len() != remaining.len();
    let remaining = trimmed_remaining;
    let title = if remaining.is_empty() {
        None
    } else {
        if !had_separator {
            return None;
        }
        let (title, consumed) = parse_ref_title_and_consumed(remaining, pedantic)?;
        if !remaining[consumed..].trim().is_empty() {
            return None;
        }
        Some(title)
    };

    Some((key, ReferenceDefinition { href, title }))
}

fn join_reference_definition_lines(lines: &[&str], start: usize, end: usize) -> String {
    let mut out = String::new();
    for idx in start..=end {
        let line = lines[idx].trim_end_matches('\r');
        if idx > start {
            let prev = lines[idx - 1].trim_end_matches('\r');
            if !prev.ends_with('\\') {
                out.push('\n');
            }
        }
        if line.ends_with('\\') && idx < end {
            out.push_str(&line[..line.len() - 1]);
        } else {
            out.push_str(line);
        }
    }
    out
}

fn skip_reference_whitespace(raw: &str) -> Option<&str> {
    let trimmed = raw.trim_start_matches([' ', '\t', '\n', '\r']);
    if trimmed.len() == raw.len() {
        return Some(raw);
    }
    if raw.contains("\n\n") || raw.contains("\r\n\r\n") {
        return None;
    }
    Some(trimmed)
}

fn parse_reference_destination(raw: &str) -> Option<(String, &str)> {
    if let Some(rest) = raw.strip_prefix('<') {
        let end = find_unescaped_char(rest, '>')?;
        let href = normalize_reference_destination(&rest[..end])?;
        return Some((href, &rest[end + 1..]));
    }

    let mut escaped = false;
    for (idx, ch) in raw.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch.is_whitespace() {
            let href = normalize_reference_destination(&raw[..idx])?;
            return Some((href, &raw[idx..]));
        }
    }

    Some((normalize_reference_destination(raw)?, ""))
}

fn parse_ref_title_and_consumed(raw: &str, pedantic: bool) -> Option<(String, usize)> {
    let raw = raw.trim_start();
    let opener = raw.chars().next()?;
    let quote_end = match opener {
        '"' | '\'' => opener,
        '(' => ')',
        _ => return None,
    };

    let end = if pedantic && matches!(opener, '"' | '\'') {
        find_last_unescaped_reference_title_close(raw, quote_end)?
    } else {
        find_first_unescaped_reference_title_close(raw, quote_end)?
    };

    let opener_len = opener.len_utf8();
    let title = decode_html_entities(&unescape_reference_text(&raw[opener_len..end]));
    let consumed = end + quote_end.len_utf8();
    Some((title, consumed))
}

fn find_first_unescaped_reference_title_close(raw: &str, quote_end: char) -> Option<usize> {
    let mut chars = raw.char_indices();
    chars.next()?;

    while let Some((idx, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        if ch == quote_end {
            return Some(idx);
        }
    }

    None
}

fn find_last_unescaped_reference_title_close(raw: &str, quote_end: char) -> Option<usize> {
    let mut chars = raw.char_indices();
    chars.next()?;

    let mut candidate = None;
    while let Some((idx, ch)) = chars.next() {
        if ch == '\\' {
            chars.next();
            continue;
        }
        if ch == quote_end {
            candidate = Some(idx);
        }
    }

    candidate
}

pub(crate) fn normalize_reference_destination(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return Some(String::new());
    }

    let mut unescaped = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if is_reference_escapable(next) {
                    unescaped.push(next);
                } else {
                    unescaped.push('\\');
                    unescaped.push(next);
                }
                continue;
            }
            unescaped.push('\\');
            continue;
        }
        unescaped.push(ch);
    }

    Some(percent_encode_reference_destination(&decode_html_entities(
        &unescaped,
    )))
}

fn percent_encode_reference_destination(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '\\' => out.push_str("%5C"),
            '[' => out.push_str("%5B"),
            ']' => out.push_str("%5D"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            _ if ch.is_ascii() && !ch.is_ascii_control() => out.push(ch),
            _ => {
                let mut buf = [0u8; 4];
                for byte in ch.encode_utf8(&mut buf).as_bytes() {
                    use std::fmt::Write as _;
                    write!(out, "%{:02X}", byte).expect("write percent-encoded byte");
                }
            }
        }
    }
    out
}

pub(crate) fn decode_html_entities(raw: &str) -> String {
    // Fast path: no '&' means no entities to decode
    if !raw.contains('&') {
        return raw.to_string();
    }

    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;

    while i < raw.len() {
        let tail = &raw[i..];
        if let Some((decoded, consumed)) = parse_html_entity(tail) {
            out.push_str(&decoded);
            i += consumed;
            continue;
        }

        let Some(ch) = tail.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

pub(crate) fn parse_html_entity(raw: &str) -> Option<(String, usize)> {
    let rest = raw.strip_prefix('&')?;
    let end = rest.find(';')?;
    let decoded = decode_entity(&rest[..end])?;
    Some((decoded, end + 2))
}

fn decode_entity(entity: &str) -> Option<String> {
    if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        if hex.is_empty() || hex.len() > 6 {
            return None;
        }
        return Some(decode_numeric_entity(u32::from_str_radix(hex, 16).ok()?));
    }
    if let Some(dec) = entity.strip_prefix('#') {
        if dec.is_empty() || dec.len() > 7 {
            return None;
        }
        return Some(decode_numeric_entity(dec.parse::<u32>().ok()?));
    }

    let decoded = match entity {
        "quot" => "\"",
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "apos" => "'",
        "nbsp" => "\u{00A0}",
        "copy" => "\u{00A9}",
        "AElig" => "\u{00C6}",
        "Dcaron" => "\u{010E}",
        "frac34" => "\u{00BE}",
        "HilbertSpace" => "\u{210B}",
        "DifferentialD" => "\u{2146}",
        "ClockwiseContourIntegral" => "\u{2232}",
        "ngE" => "\u{2267}\u{0338}",
        "auml" => "ä",
        "Auml" => "Ä",
        "ouml" => "ö",
        "Ouml" => "Ö",
        "uuml" => "ü",
        "Uuml" => "Ü",
        "szlig" => "ß",
        _ => return None,
    };

    Some(decoded.to_string())
}

fn decode_numeric_entity(codepoint: u32) -> String {
    let ch = match codepoint {
        0 => '\u{FFFD}',
        0xD800..=0xDFFF => '\u{FFFD}',
        0x110000..=u32::MAX => '\u{FFFD}',
        _ => char::from_u32(codepoint).unwrap_or('\u{FFFD}'),
    };
    ch.to_string()
}

fn unescape_reference_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if is_reference_escapable(next) {
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

fn find_unescaped_char(input: &str, marker: char) -> Option<usize> {
    let mut escaped = false;
    for (idx, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == marker {
            return Some(idx);
        }
    }
    None
}

fn is_reference_escapable(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

fn parse_paragraph(
    lines: &[&str],
    start: usize,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
    allow_ref_defs: bool,
    preserve_leading_indent: bool,
) -> (Vec<Inline>, usize) {
    let mut acc = String::new();
    let mut i = start;

    while i < lines.len() {
        let raw_line = strip_lazy_prefix(lines[i]);
        if raw_line.trim().is_empty() {
            break;
        }
        if allow_ref_defs
            && acc.is_empty()
            && parse_reference_definition(raw_line, pedantic).is_some()
        {
            break;
        }

        if parse_atx_heading(raw_line, pedantic).is_some()
            || parse_thematic_break(raw_line).is_some()
            || parse_fenced_code_block(lines, i).is_some()
            || (list_interrupts_paragraph(raw_line)
                && parse_list_block(lines, i, gfm, pedantic, refs).is_some())
            || (line_could_start_blockquote(raw_line)
                && parse_blockquote_block(lines, i).is_some())
            || parse_table_block(lines, i, gfm, pedantic, refs).is_some()
            || html_block_interrupts_paragraph(lines, i)
        {
            break;
        }

        if pedantic
            && !acc.is_empty()
            && lines
                .get(i + 1)
                .and_then(|next_line| parse_setext_heading(raw_line, next_line))
                .is_some()
        {
            break;
        }

        let line = normalize_paragraph_indent(raw_line, preserve_leading_indent);
        if !acc.is_empty() {
            acc.push('\n');
        }
        acc.push_str(line);
        i += 1;
    }

    if acc.is_empty() {
        if i == start {
            return (
                parse_block_inlines(lines[start], gfm, pedantic, refs),
                start + 1,
            );
        }
        let trimmed = acc.trim_end_matches([' ', '\t']);
        return (parse_block_inlines(trimmed, gfm, pedantic, refs), i);
    }

    let trimmed = acc.trim_end_matches([' ', '\t']);
    (parse_block_inlines(trimmed, gfm, pedantic, refs), i)
}

fn html_block_interrupts_paragraph(lines: &[&str], i: usize) -> bool {
    let Some(line) = lines.get(i) else {
        return false;
    };
    let (indent, line) = split_leading_ws(line);
    if indent > 3 {
        return false;
    }
    let line = line.trim_start();
    if !line.starts_with('<') {
        return false;
    }

    if line.starts_with("<!--")
        || line.starts_with("<?")
        || line.starts_with("<![CDATA[")
        || line.starts_with("<!")
            && line
                .as_bytes()
                .get(2)
                .is_some_and(|b| b.is_ascii_uppercase())
    {
        return true;
    }

    if let Some(tag_name) = parse_closing_html_tag_name(line) {
        return is_block_html_tag(tag_name);
    }

    if let Some(tag_name) = parse_html_tag_name(line) {
        return is_block_html_tag(tag_name)
            || matches!(tag_name, "script" | "pre" | "style" | "textarea");
    }

    false
}

fn normalize_paragraph_indent(line: &str, preserve_leading_indent: bool) -> &str {
    let line = strip_forced_paragraph_prefix(line);
    if preserve_leading_indent {
        line
    } else {
        trim_paragraph_line(line)
    }
}

fn trim_setext_heading_line(line: &str) -> &str {
    trim_paragraph_line(strip_forced_paragraph_prefix(line)).trim_end_matches([' ', '\t'])
}

fn trim_paragraph_line(line: &str) -> &str {
    line.trim_start_matches([' ', '\t'])
}

fn parse_block_inlines(
    line: &str,
    gfm: bool,
    pedantic: bool,
    refs: &mut HashMap<String, ReferenceDefinition>,
) -> Vec<Inline> {
    let normalized = normalize_block_inline_input(line);
    crate::markdown::inline::InlineParser::with_refs(normalized.as_ref(), gfm, pedantic, refs)
        .parse()
}

fn normalize_block_inline_input(line: &str) -> Cow<'_, str> {
    if !line.contains(LAZY_QUOTE_PREFIX) && !line.contains(FORCED_PARAGRAPH_PREFIX) {
        return Cow::Borrowed(line);
    }

    if !line.contains('\n') {
        return Cow::Borrowed(strip_forced_paragraph_prefix(strip_lazy_prefix(line)));
    }

    let mut out = String::with_capacity(line.len());
    for raw_line in line.lines() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(strip_forced_paragraph_prefix(strip_lazy_prefix(raw_line)));
    }
    Cow::Owned(out)
}

fn force_paragraph_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + FORCED_PARAGRAPH_PREFIX.len_utf8());
    out.push(FORCED_PARAGRAPH_PREFIX);
    out.push_str(line);
    out
}

fn strip_forced_paragraph_prefix(line: &str) -> &str {
    line.strip_prefix(FORCED_PARAGRAPH_PREFIX).unwrap_or(line)
}

fn strip_lazy_prefix(line: &str) -> &str {
    line.strip_prefix(LAZY_QUOTE_PREFIX).unwrap_or(line)
}

fn line_has_list_marker_with_content(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(marker) = list_marker(trimmed) else {
        return false;
    };

    if let Some(rest) = trimmed.get(marker.end..) {
        return !rest.trim_start().is_empty();
    }
    true
}

#[inline]
fn line_could_start_blockquote(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && i < 4 && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    i < bytes.len() && bytes[i] == b'>'
}

fn list_interrupts_paragraph(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(marker) = list_marker(trimmed) else {
        return false;
    };
    if marker.ordered && marker.start != 1 {
        return false;
    }
    line_has_list_marker_with_content(trimmed)
}
