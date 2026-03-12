use std::collections::HashMap;

use crate::markdown::{
    ast::inline::Inline,
    block::{
        ReferenceDefinition, decode_html_entities, normalize_reference_destination,
        normalize_reference_label, parse_html_entity, try_normalize_reference_label,
    },
};

pub(crate) struct InlineParser<'a> {
    input: &'a str,
    gfm: bool,
    pedantic: bool,
    refs: Option<&'a HashMap<String, ReferenceDefinition>>,
}

impl<'a> InlineParser<'a> {
    #[cfg(test)]
    pub(crate) fn new(input: &'a str, gfm: bool, pedantic: bool) -> Self {
        Self {
            input,
            gfm,
            pedantic,
            refs: None,
        }
    }

    pub(crate) fn with_refs(
        input: &'a str,
        gfm: bool,
        pedantic: bool,
        refs: &'a HashMap<String, ReferenceDefinition>,
    ) -> Self {
        Self {
            input,
            gfm,
            pedantic,
            refs: Some(refs),
        }
    }

    pub(crate) fn parse(&self) -> Vec<Inline> {
        match self.refs {
            Some(refs) => parse_inline_with_refs(self.input, self.gfm, self.pedantic, Some(refs)),
            None => parse_inline_with_refs(self.input, self.gfm, self.pedantic, None),
        }
    }
}

#[cfg(test)]
pub(crate) fn parse_inline(input: &str, gfm: bool) -> Vec<Inline> {
    InlineParser::new(input, gfm, false).parse()
}

pub(crate) fn parse_inline_with_refs(
    input: &str,
    gfm: bool,
    pedantic: bool,
    refs: Option<&HashMap<String, ReferenceDefinition>>,
) -> Vec<Inline> {
    parse_inline_with_refs_mode(input, gfm, pedantic, refs, gfm)
}

fn parse_inline_with_refs_mode(
    input: &str,
    gfm: bool,
    pedantic: bool,
    refs: Option<&HashMap<String, ReferenceDefinition>>,
    allow_bare_autolinks: bool,
) -> Vec<Inline> {
    if input.is_empty() {
        return Vec::new();
    }

    if !inline_fragment_needs_parse(input) {
        let text = normalize_inline_plain_text(input.to_string());
        if allow_bare_autolinks && gfm {
            return autolink_text_nodes(text);
        }
        return vec![Inline::Text(text)];
    }

    let mut chars = Vec::with_capacity(input.len());
    chars.extend(input.chars());
    let mut out: Vec<InlinePart> = Vec::with_capacity((chars.len() / 4).max(8));
    let mut has_delimiters = false;
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '\n' {
            let mut back = i;
            while back > 0 && (chars[back - 1] == ' ' || chars[back - 1] == '\t') {
                back -= 1;
            }
            if i - back >= 2 {
                let mut to_remove = i - back;
                while to_remove > 0 {
                    match out.last_mut() {
                        Some(InlinePart::Node(Inline::Text(last))) => {
                            if last.ends_with(' ') {
                                last.pop();
                                to_remove -= 1;
                                continue;
                            }
                            if last.ends_with('\t') {
                                last.pop();
                                to_remove -= 1;
                                continue;
                            }
                            break;
                        }
                        _ => break,
                    }
                }
                push_inline_part(&mut out, InlinePart::Node(Inline::HardBreak));
                i += 1;
                continue;
            }

            if i + 1 < chars.len() && chars[i + 1] == '<' {
                push_inline_text_char(&mut out, '\n');
                i += 1;
                continue;
            }

            push_inline_part(&mut out, InlinePart::Node(Inline::SoftBreak));
            i += 1;
            continue;
        }

        if chars[i] == '\\' {
            if i + 1 < chars.len() {
                if chars[i + 1] == '\n' {
                    push_inline_part(&mut out, InlinePart::Node(Inline::HardBreak));
                    i += 2;
                    continue;
                }

                if is_escapable(chars[i + 1]) {
                    push_inline_part(&mut out, inline_text_part_from_char(chars[i + 1]));
                    i += 2;
                    continue;
                }
            }
            push_inline_text_char(&mut out, '\\');
            i += 1;
            continue;
        }

        if chars[i] == '<' {
            if let Some((href, label, close)) = parse_autolink_like(&chars, i) {
                push_inline_part(
                    &mut out,
                    InlinePart::Node(Inline::Link {
                        label: vec![Inline::Text(label)],
                        href,
                        title: None,
                    }),
                );
                i = close + 1;
                continue;
            }
        }

        if chars[i] == '<' {
            if let Some(close) = parse_raw_html(&chars, i) {
                push_inline_part(
                    &mut out,
                    InlinePart::Node(Inline::RawHtml(chars_to_string(&chars[i..close]))),
                );
                i = close;
                continue;
            }
        }

        if let Some((href, label, close)) = parse_quoted_autolink_like(&chars, i) {
            push_inline_part(&mut out, inline_text_part_from_char(chars[i]));
            push_inline_part(
                &mut out,
                InlinePart::Node(Inline::Link {
                    label: vec![Inline::Text(label)],
                    href,
                    title: None,
                }),
            );
            i = close;
            continue;
        }

        if let Some((delimiter, run_len)) = parse_delimiter_run(&chars, i, gfm) {
            has_delimiters |= matches!(delimiter, InlinePart::Delimiter { .. });
            push_inline_part(&mut out, delimiter);
            i += run_len;
            continue;
        }

        if chars[i] == '`' {
            let open_len = count_consecutive(&chars, i, '`');
            if open_len > 1 && parse_code_span(&chars, i).is_none() {
                push_inline_part(
                    &mut out,
                    InlinePart::Node(Inline::Text(chars_to_string(&chars[i..i + open_len]))),
                );
                i += open_len;
                continue;
            }
            if let Some((code, close)) = parse_code_span(&chars, i) {
                push_inline_part(&mut out, InlinePart::Node(Inline::Code(code)));
                i = close + 1;
                continue;
            }
        }

        if chars[i] == '!'
            && i + 1 < chars.len()
            && chars[i + 1] == '['
            && !is_escaped_char(&chars, i)
        {
            if let Some((close_ref, src, title, alt)) =
                parse_reference_image(&chars, i + 1, gfm, pedantic, refs)
            {
                let parsed_alt = parse_inline_fragment(&alt, gfm, pedantic, refs);
                push_inline_part(
                    &mut out,
                    InlinePart::Node(Inline::Image {
                        alt: parsed_alt,
                        src,
                        title,
                    }),
                );
                i = close_ref + 1;
                continue;
            }

            if let Some((src, alt, title, close_src)) = parse_image_like(&chars, i + 1, pedantic) {
                let parsed_alt = parse_inline_fragment(&alt, gfm, pedantic, refs);
                push_inline_part(
                    &mut out,
                    InlinePart::Node(Inline::Image {
                        alt: parsed_alt,
                        src,
                        title,
                    }),
                );
                i = close_src + 1;
                continue;
            }
        }

        if chars[i] == '[' && !is_unescaped_image_marker(&chars, i) {
            if let Some((href, close_link, label, title)) = parse_link_like(&chars, i, pedantic) {
                let parsed_label = parse_inline_fragment(&label, gfm, pedantic, refs);
                if inline_nodes_contain_link(&parsed_label) {
                    // Links cannot contain other links. Fall back to literal text so inner links survive.
                } else {
                    push_inline_part(
                        &mut out,
                        InlinePart::Node(Inline::Link {
                            label: parsed_label,
                            href,
                            title,
                        }),
                    );
                    i = close_link + 1;
                    continue;
                }
            }

            if let Some((close_link, href, title, label)) =
                parse_reference_link(&chars, i, gfm, pedantic, refs)
            {
                let parsed_label = parse_inline_fragment(&label, gfm, pedantic, refs);
                if inline_nodes_contain_link(&parsed_label) {
                    // Reference links follow the same no-links-inside-links rule.
                } else {
                    push_inline_part(
                        &mut out,
                        InlinePart::Node(Inline::Link {
                            label: parsed_label,
                            href,
                            title,
                        }),
                    );
                    i = close_link + 1;
                    continue;
                }
            }
        }

        if chars[i] == '&' {
            if let Some((decoded, consumed_chars)) = parse_html_entity_chars(&chars, i) {
                push_inline_part(&mut out, InlinePart::Node(Inline::Text(decoded)));
                i += consumed_chars;
                continue;
            }
        }

        let mut plain = String::with_capacity((chars.len() - i).min(32));
        while i < chars.len() {
            if chars[i] == '\\' && i + 1 < chars.len() && is_escapable(chars[i + 1]) {
                plain.push(chars[i + 1]);
                i += 2;
                continue;
            }

            if chars[i] == '\n' || is_token_start(&chars, i) {
                break;
            }

            plain.push(chars[i]);
            i += 1;
        }
        if plain.is_empty() && i < chars.len() {
            plain.push(chars[i]);
            i += 1;
        }
        push_inline_part(
            &mut out,
            InlinePart::Node(Inline::Text(normalize_inline_plain_text(plain))),
        );
    }

    let nodes = if has_delimiters {
        resolve_inline_parts(out)
    } else {
        inline_parts_into_nodes(out)
    };

    if allow_bare_autolinks && gfm {
        apply_gfm_bare_autolinks(nodes)
    } else {
        nodes
    }
}

#[inline]
fn is_token_start(chars: &[char], i: usize) -> bool {
    let c = chars[i];
    matches!(
        c,
        '\\' | '*' | '_' | '[' | '!' | '`' | '\n' | '~' | '>' | '<' | '&'
    )
}

fn inline_nodes_contain_link(nodes: &[Inline]) -> bool {
    nodes.iter().any(|node| match node {
        Inline::Link { .. } => true,
        Inline::Em(children) | Inline::Strong(children) | Inline::Del(children) => {
            inline_nodes_contain_link(children)
        }
        Inline::Image { alt, .. } => inline_nodes_contain_link(alt),
        _ => false,
    })
}

fn parse_inline_fragment(
    input: &str,
    gfm: bool,
    pedantic: bool,
    refs: Option<&HashMap<String, ReferenceDefinition>>,
) -> Vec<Inline> {
    if input.is_empty() {
        return Vec::new();
    }

    if !inline_fragment_needs_parse(input) {
        return vec![Inline::Text(normalize_inline_plain_text(input.to_string()))];
    }

    parse_inline_with_refs_mode(input, gfm, pedantic, refs, false)
}

#[inline]
fn inline_fragment_needs_parse(input: &str) -> bool {
    input.as_bytes().iter().any(|byte| {
        matches!(
            *byte,
            92 | b'*' | b'_' | b'[' | b'!' | b'`' | 10 | 13 | b'~' | b'>' | b'<' | b'&'
        )
    })
}

#[derive(Clone)]
enum InlinePart {
    Node(Inline),
    Delimiter {
        marker: char,
        len: usize,
        original_len: usize,
        can_open: bool,
        can_close: bool,
    },
}

#[inline]
fn push_inline_part(out: &mut Vec<InlinePart>, part: InlinePart) {
    match part {
        InlinePart::Node(Inline::Text(text)) => {
            if text.is_empty() {
                return;
            }
            if let Some(InlinePart::Node(Inline::Text(last))) = out.last_mut() {
                last.push_str(&text);
            } else {
                out.push(InlinePart::Node(Inline::Text(text)));
            }
        }
        _ => out.push(part),
    }
}

fn inline_text_part_from_char(ch: char) -> InlinePart {
    let mut text = String::with_capacity(ch.len_utf8());
    text.push(ch);
    InlinePart::Node(Inline::Text(text))
}

#[inline]
fn push_inline_text_char(out: &mut Vec<InlinePart>, ch: char) {
    push_inline_part(out, inline_text_part_from_char(ch));
}

fn parse_delimiter_run(chars: &[char], start: usize, gfm: bool) -> Option<(InlinePart, usize)> {
    let marker = chars.get(start).copied()?;
    if marker != '*' && marker != '_' && !(gfm && marker == '~') {
        return None;
    }

    let run_len = count_consecutive(chars, start, marker);
    if marker == '~' && run_len > 2 {
        return Some((
            InlinePart::Node(Inline::Text(chars_to_string(
                &chars[start..start + run_len],
            ))),
            run_len,
        ));
    }
    let can_open = delimiter_run_can_open(chars, start, run_len, marker);
    let can_close = delimiter_run_can_close(chars, start, run_len, marker);
    let part = if can_open || can_close {
        InlinePart::Delimiter {
            marker,
            len: run_len,
            original_len: run_len,
            can_open,
            can_close,
        }
    } else {
        InlinePart::Node(Inline::Text(chars_to_string(
            &chars[start..start + run_len],
        )))
    };

    Some((part, run_len))
}

fn inline_parts_into_nodes(parts: Vec<InlinePart>) -> Vec<Inline> {
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        match part {
            InlinePart::Node(node) => push_inline_node(&mut out, node),
            InlinePart::Delimiter { marker, len, .. } => {
                let mut s = String::with_capacity(len);
                for _ in 0..len {
                    s.push(marker);
                }
                push_inline_node(&mut out, Inline::Text(s));
            }
        }
    }
    out
}

fn resolve_inline_parts(parts: Vec<InlinePart>) -> Vec<Inline> {
    inline_parts_into_nodes(resolve_delimiter_runs(parts))
}

#[inline]
fn push_inline_node(out: &mut Vec<Inline>, node: Inline) {
    match node {
        Inline::Text(text) => {
            if text.is_empty() {
                return;
            }
            if let Some(Inline::Text(last)) = out.last_mut() {
                last.push_str(&text);
            } else {
                out.push(Inline::Text(text));
            }
        }
        _ => out.push(node),
    }
}

#[derive(Clone, Copy)]
struct TextAtom {
    raw_start: usize,
    raw_end: usize,
    ch: char,
}

struct BareAutolinkCandidate {
    end: usize,
    href: String,
}

fn apply_gfm_bare_autolinks(nodes: Vec<Inline>) -> Vec<Inline> {
    if !nodes_may_have_bare_autolinks(&nodes) {
        return nodes;
    }
    let mut stack = Vec::new();
    apply_gfm_bare_autolinks_with_stack(nodes, &mut stack)
}

fn nodes_may_have_bare_autolinks(nodes: &[Inline]) -> bool {
    nodes.iter().any(|node| match node {
        Inline::Text(text) => text_may_have_bare_autolink(text),
        Inline::Em(children) | Inline::Strong(children) | Inline::Del(children) => {
            nodes_may_have_bare_autolinks(children)
        }
        _ => false,
    })
}

fn apply_gfm_bare_autolinks_with_stack(nodes: Vec<Inline>, stack: &mut Vec<String>) -> Vec<Inline> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            Inline::RawHtml(html) => {
                update_inline_html_stack(&html, stack);
                out.push(Inline::RawHtml(html));
            }
            Inline::Text(text) => {
                if should_skip_bare_autolink(stack) {
                    push_inline_node(&mut out, Inline::Text(text));
                } else {
                    for node in autolink_text_nodes(text) {
                        push_inline_node(&mut out, node);
                    }
                }
            }
            Inline::Em(children) => {
                if should_skip_bare_autolink(stack) {
                    out.push(Inline::Em(children));
                } else {
                    out.push(Inline::Em(apply_gfm_bare_autolinks_with_stack(
                        children, stack,
                    )));
                }
            }
            Inline::Strong(children) => {
                if should_skip_bare_autolink(stack) {
                    out.push(Inline::Strong(children));
                } else {
                    out.push(Inline::Strong(apply_gfm_bare_autolinks_with_stack(
                        children, stack,
                    )));
                }
            }
            Inline::Del(children) => {
                if should_skip_bare_autolink(stack) {
                    out.push(Inline::Del(children));
                } else {
                    out.push(Inline::Del(apply_gfm_bare_autolinks_with_stack(
                        children, stack,
                    )));
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn autolink_text_nodes(text: String) -> Vec<Inline> {
    if text.is_empty() || !text_may_have_bare_autolink(&text) {
        return vec![Inline::Text(text)];
    }

    let atoms = text_atoms(&text);
    if atoms.is_empty() {
        return vec![Inline::Text(text)];
    }

    let mut out = Vec::new();
    let mut last_raw = 0usize;
    let mut i = 0usize;
    let mut found = false;

    while i < atoms.len() {
        let Some(candidate) = parse_bare_autolink_candidate(&atoms, i) else {
            i += 1;
            continue;
        };

        let raw_start = atoms[i].raw_start;
        let raw_end = atoms[candidate.end - 1].raw_end;
        if raw_start > last_raw {
            push_inline_node(
                &mut out,
                Inline::Text(text[last_raw..raw_start].to_string()),
            );
        }

        let label = text[raw_start..raw_end].to_string();
        out.push(Inline::Link {
            label: vec![Inline::Text(label)],
            href: candidate.href,
            title: None,
        });
        last_raw = raw_end;
        i = candidate.end;
        found = true;
    }

    if !found {
        return vec![Inline::Text(text)];
    }

    if last_raw < text.len() {
        push_inline_node(&mut out, Inline::Text(text[last_raw..].to_string()));
    }

    out
}

fn text_may_have_bare_autolink(text: &str) -> bool {
    text.as_bytes()
        .iter()
        .any(|byte| matches!(*byte, b':' | b'@' | b'.'))
}

fn should_skip_bare_autolink(stack: &[String]) -> bool {
    stack.iter().any(|tag| {
        matches!(
            tag.as_str(),
            "a" | "code" | "pre" | "script" | "style" | "textarea"
        )
    })
}

fn update_inline_html_stack(tag: &str, stack: &mut Vec<String>) {
    if tag.starts_with("<!--") || tag.starts_with("<!") || tag.starts_with("<?") {
        return;
    }

    let Some(name) = parse_inline_html_tag_name(tag.trim_start_matches('<').trim_end_matches('>'))
    else {
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

    if !self_closing && !is_inline_html_void_tag(&name) {
        stack.push(name);
    }
}

fn parse_inline_html_tag_name(tag_body: &str) -> Option<String> {
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

fn is_inline_html_void_tag(name: &str) -> bool {
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

fn text_atoms(text: &str) -> Vec<TextAtom> {
    let mut atoms = Vec::with_capacity(text.len());
    let mut indices = text.char_indices().peekable();
    while let Some((start, ch)) = indices.next() {
        let end = indices.peek().map(|(idx, _)| *idx).unwrap_or(text.len());
        atoms.push(TextAtom {
            raw_start: start,
            raw_end: end,
            ch,
        });
    }
    atoms
}

fn parse_bare_autolink_candidate(
    atoms: &[TextAtom],
    start: usize,
) -> Option<BareAutolinkCandidate> {
    if !bare_autolink_start_boundary(atoms, start) {
        return None;
    }

    parse_bare_url_candidate(atoms, start).or_else(|| parse_bare_email_candidate(atoms, start))
}

fn bare_autolink_start_boundary(atoms: &[TextAtom], start: usize) -> bool {
    if start == 0 {
        return true;
    }
    !matches!(
        atoms[start - 1].ch,
        'a'..='z' | 'A'..='Z' | '0'..='9' | '@' | '.' | '_' | '-' | '/' | ':'
    )
}

fn parse_bare_url_candidate(atoms: &[TextAtom], start: usize) -> Option<BareAutolinkCandidate> {
    if starts_with_www_atoms(atoms, start) {
        let end = trim_generic_url_end_atoms(atoms, start, scan_url_end_atoms(atoms, start));
        if end <= start + 4 {
            return None;
        }
        return Some(BareAutolinkCandidate {
            end,
            href: format!("http://{}", collect_decoded_atoms(atoms, start, end)),
        });
    }

    let (scheme_end, scheme) = parse_scheme_prefix_atoms(atoms, start)?;
    if scheme_end >= atoms.len() || atoms[scheme_end].ch.is_whitespace() {
        return None;
    }

    if scheme.eq_ignore_ascii_case("mailto") || scheme.eq_ignore_ascii_case("xmpp") {
        return parse_emailish_scheme_candidate_atoms(atoms, start, scheme_end + 1, &scheme);
    }

    let end = trim_generic_url_end_atoms(atoms, start, scan_url_end_atoms(atoms, start));
    if end <= scheme_end + 1 {
        return None;
    }

    Some(BareAutolinkCandidate {
        end,
        href: collect_decoded_atoms(atoms, start, end),
    })
}

fn parse_emailish_scheme_candidate_atoms(
    atoms: &[TextAtom],
    start: usize,
    body_start: usize,
    scheme: &str,
) -> Option<BareAutolinkCandidate> {
    let email_end = parse_email_body_atoms(atoms, body_start)?;
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

    Some(BareAutolinkCandidate {
        end,
        href: collect_decoded_atoms(atoms, start, end),
    })
}

fn parse_bare_email_candidate(atoms: &[TextAtom], start: usize) -> Option<BareAutolinkCandidate> {
    let end = parse_email_body_atoms(atoms, start)?;
    if matches!(atoms.get(end).map(|atom| atom.ch), Some('-' | '_')) {
        return None;
    }

    Some(BareAutolinkCandidate {
        end,
        href: format!("mailto:{}", collect_decoded_atoms(atoms, start, end)),
    })
}

fn parse_email_body_atoms(atoms: &[TextAtom], start: usize) -> Option<usize> {
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

fn starts_with_www_atoms(atoms: &[TextAtom], start: usize) -> bool {
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

fn parse_scheme_prefix_atoms(atoms: &[TextAtom], start: usize) -> Option<(usize, String)> {
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

    Some((i, collect_decoded_atoms(atoms, start, i)))
}

fn scan_url_end_atoms(atoms: &[TextAtom], start: usize) -> usize {
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

fn trim_generic_url_end_atoms(atoms: &[TextAtom], start: usize, mut end: usize) -> usize {
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
            if let Some(entity_start) = entity_like_suffix_start_atoms(atoms, start, end) {
                end = entity_start;
            } else {
                end -= 1;
            }
            trimmed = true;
        } else if last == ')' && unmatched_closing_parens_atoms(atoms, start, end) > 0 {
            end -= 1;
            trimmed = true;
        }

        if !trimmed {
            break;
        }
    }

    end
}

fn entity_like_suffix_start_atoms(atoms: &[TextAtom], start: usize, end: usize) -> Option<usize> {
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

fn unmatched_closing_parens_atoms(atoms: &[TextAtom], start: usize, end: usize) -> usize {
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

fn collect_decoded_atoms(atoms: &[TextAtom], start: usize, end: usize) -> String {
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

fn resolve_delimiter_runs(mut parts: Vec<InlinePart>) -> Vec<InlinePart> {
    loop {
        let mut matched = None;

        for closer_idx in 0..parts.len() {
            let InlinePart::Delimiter {
                marker,
                len: closer_len,
                original_len: closer_original_len,
                can_open: closer_can_open,
                can_close,
            } = &parts[closer_idx]
            else {
                continue;
            };

            if !can_close {
                continue;
            }

            let Some((opener_idx, use_len)) = find_matching_opener(
                &parts,
                closer_idx,
                *marker,
                *closer_len,
                *closer_original_len,
                *closer_can_open,
            ) else {
                continue;
            };

            matched = Some((opener_idx, closer_idx, *marker, use_len));
            break;
        }

        let Some((opener_idx, closer_idx, marker, use_len)) = matched else {
            return parts;
        };

        let after = parts.split_off(closer_idx + 1);
        let mut inner_and_closer = parts.split_off(opener_idx + 1);
        let opener = parts.pop().expect("matched opener must exist");
        let closer = inner_and_closer.pop().expect("matched closer must exist");

        let InlinePart::Delimiter {
            len: opener_len,
            original_len: opener_original_len,
            can_open: opener_can_open,
            can_close: opener_can_close,
            ..
        } = opener
        else {
            unreachable!("matched opener must be delimiter");
        };

        let InlinePart::Delimiter {
            len: closer_len,
            original_len: closer_original_len,
            can_open: closer_can_open,
            can_close: closer_can_close,
            ..
        } = closer
        else {
            unreachable!("matched closer must be delimiter");
        };

        let inner = resolve_inline_parts(inner_and_closer);
        let wrapped = match (marker, use_len) {
            ('~', _) => Inline::Del(inner),
            (_, 2) => Inline::Strong(inner),
            _ => Inline::Em(inner),
        };

        if opener_len > use_len {
            parts.push(InlinePart::Delimiter {
                marker,
                len: opener_len - use_len,
                original_len: opener_original_len,
                can_open: opener_can_open,
                can_close: opener_can_close,
            });
        }

        parts.push(InlinePart::Node(wrapped));

        if closer_len > use_len {
            parts.push(InlinePart::Delimiter {
                marker,
                len: closer_len - use_len,
                original_len: closer_original_len,
                can_open: closer_can_open,
                can_close: closer_can_close,
            });
        }

        parts.extend(after);
    }
}

fn find_matching_opener(
    parts: &[InlinePart],
    closer_idx: usize,
    marker: char,
    closer_len: usize,
    closer_original_len: usize,
    closer_can_open: bool,
) -> Option<(usize, usize)> {
    for opener_idx in (0..closer_idx).rev() {
        let InlinePart::Delimiter {
            marker: opener_marker,
            len: opener_len,
            original_len: opener_original_len,
            can_open,
            can_close: opener_can_close,
        } = &parts[opener_idx]
        else {
            continue;
        };

        if !can_open || *opener_marker != marker {
            continue;
        }

        if !delimiter_runs_can_pair(
            marker,
            *opener_len,
            *opener_original_len,
            *opener_can_close,
            closer_len,
            closer_original_len,
            closer_can_open,
        ) {
            continue;
        }

        if should_defer_ambiguous_closer(
            parts,
            opener_idx,
            closer_idx,
            marker,
            *opener_len,
            *opener_original_len,
            *opener_can_close,
            closer_len,
            closer_original_len,
            closer_can_open,
        ) {
            continue;
        }

        let Some(use_len) = delimiter_use_len(marker, *opener_len, closer_len) else {
            continue;
        };
        return Some((opener_idx, use_len));
    }

    None
}

fn should_defer_ambiguous_closer(
    parts: &[InlinePart],
    opener_idx: usize,
    closer_idx: usize,
    marker: char,
    opener_len: usize,
    opener_original_len: usize,
    opener_can_close: bool,
    _closer_len: usize,
    _closer_original_len: usize,
    closer_can_open: bool,
) -> bool {
    if !closer_can_open || opener_can_close {
        return false;
    }

    let has_earlier_opener = parts[..opener_idx].iter().any(|part| match part {
        InlinePart::Delimiter {
            marker: earlier_marker,
            can_open,
            ..
        } => *earlier_marker == marker && *can_open,
        _ => false,
    });

    if has_earlier_opener {
        return false;
    }

    parts
        .iter()
        .enumerate()
        .skip(closer_idx + 1)
        .any(|(_, part)| match part {
            InlinePart::Delimiter {
                marker: later_marker,
                len: later_len,
                original_len: later_original_len,
                can_open: later_can_open,
                can_close: later_can_close,
            } if *later_marker == marker && *later_can_close => delimiter_runs_can_pair(
                marker,
                opener_len,
                opener_original_len,
                opener_can_close,
                *later_len,
                *later_original_len,
                *later_can_open,
            ),
            _ => false,
        })
}

fn delimiter_use_len(marker: char, opener_len: usize, closer_len: usize) -> Option<usize> {
    if marker == '~' {
        if opener_len >= 2 && closer_len >= 2 {
            Some(2)
        } else if opener_len == 1 && closer_len == 1 {
            Some(1)
        } else {
            None
        }
    } else if opener_len >= 2 && closer_len >= 2 {
        Some(2)
    } else {
        Some(1)
    }
}

fn delimiter_runs_can_pair(
    marker: char,
    _opener_len: usize,
    opener_original_len: usize,
    opener_can_close: bool,
    _closer_len: usize,
    closer_original_len: usize,
    closer_can_open: bool,
) -> bool {
    if marker == '~' {
        return true;
    }

    if !(opener_can_close || closer_can_open) {
        return true;
    }

    let sum = opener_original_len + closer_original_len;
    sum % 3 != 0 || (opener_original_len % 3 == 0 && closer_original_len % 3 == 0)
}

fn parse_raw_html(chars: &[char], start: usize) -> Option<usize> {
    if start + 1 >= chars.len() {
        return None;
    }

    if starts_with(chars, start, "<!--") {
        let mut i = start + 4;
        while i + 2 < chars.len() {
            if chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>' {
                return Some(i + 3);
            }
            i += 1;
        }
        return None;
    }

    if starts_with(chars, start, "<?") {
        let mut i = start + 2;
        while i + 1 < chars.len() {
            if chars[i] == '?' && chars[i + 1] == '>' {
                return Some(i + 2);
            }
            i += 1;
        }
        return None;
    }

    if starts_with(chars, start, "<![CDATA[") {
        let mut i = start + 9;
        while i + 2 < chars.len() {
            if chars[i] == ']' && chars[i + 1] == ']' && chars[i + 2] == '>' {
                return Some(i + 3);
            }
            i += 1;
        }
        return None;
    }

    if starts_with(chars, start, "<!")
        && chars
            .get(start + 2)
            .is_some_and(|ch| ch.is_ascii_uppercase())
    {
        let mut i = start + 3;
        while i < chars.len() {
            if chars[i] == '>' {
                return Some(i + 1);
            }
            i += 1;
        }
        return None;
    }

    if let Some(close) = parse_html_tag_like(chars, start) {
        return Some(close);
    }

    None
}

fn parse_html_tag_like(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    let closing = chars.get(i) == Some(&'/');
    if closing {
        i += 1;
        if i >= chars.len() || !chars[i].is_ascii_alphabetic() {
            return None;
        }
    } else if !chars.get(i)?.is_ascii_alphabetic() {
        return None;
    }

    i += 1;
    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '-') {
        i += 1;
    }

    if closing {
        while i < chars.len() && matches!(chars[i], ' ' | '\t' | '\n' | '\r') {
            i += 1;
        }
        return (chars.get(i) == Some(&'>')).then_some(i + 1);
    }

    loop {
        let mut had_space = false;
        while i < chars.len() && matches!(chars[i], ' ' | '\t' | '\n' | '\r') {
            i += 1;
            had_space = true;
        }
        if i >= chars.len() {
            return None;
        }

        match chars[i] {
            '>' => return Some(i + 1),
            '/' if chars.get(i + 1) == Some(&'>') => return Some(i + 2),
            _ => {}
        }

        if !had_space {
            return None;
        }

        if !is_html_attribute_name_start(chars[i]) {
            return None;
        }
        i += 1;
        while i < chars.len() && is_html_attribute_name_char(chars[i]) {
            i += 1;
        }

        let attr_end = i;
        while i < chars.len() && matches!(chars[i], ' ' | '\t' | '\n' | '\r') {
            i += 1;
        }
        if i >= chars.len() {
            return None;
        }
        if chars[i] != '=' {
            i = attr_end;
            continue;
        }

        i += 1;
        while i < chars.len() && matches!(chars[i], ' ' | '\t' | '\n' | '\r') {
            i += 1;
        }
        if i >= chars.len() {
            return None;
        }

        match chars[i] {
            '\'' | '"' => {
                let quote = chars[i];
                i += 1;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                if i >= chars.len() {
                    return None;
                }
                i += 1;
            }
            ' ' | '\t' | '\n' | '\r' | '>' => return None,
            _ => {
                while i < chars.len() {
                    match chars[i] {
                        ' ' | '\t' | '\n' | '\r' | '>' => break,
                        '"' | '\'' | '=' | '<' | '`' => return None,
                        _ => i += 1,
                    }
                }
            }
        }
    }
}

fn is_html_attribute_name_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || matches!(ch, '_' | ':')
}

fn is_html_attribute_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '.' | '-')
}

fn starts_with(chars: &[char], i: usize, s: &str) -> bool {
    for (offset, expected_c) in s.chars().enumerate() {
        if chars.get(i + offset) != Some(&expected_c) {
            return false;
        }
    }
    true
}

fn delimiter_run_can_open(chars: &[char], start: usize, run_len: usize, marker: char) -> bool {
    let (left_flanking, right_flanking, prev_punct, next_punct) =
        delimiter_flanking(chars, start, run_len);
    match marker {
        '_' => left_flanking && (!right_flanking || prev_punct),
        '*' => {
            let _ = next_punct;
            left_flanking
                || chars.get(start.wrapping_sub(1)).copied() == Some('~')
                || chars.get(start + run_len).copied() == Some('~')
        }
        '~' => {
            let _ = next_punct;
            left_flanking
        }
        _ => false,
    }
}

fn delimiter_run_can_close(chars: &[char], start: usize, run_len: usize, marker: char) -> bool {
    let (left_flanking, right_flanking, _prev_punct, next_punct) =
        delimiter_flanking(chars, start, run_len);
    match marker {
        '_' => right_flanking && (!left_flanking || next_punct),
        '*' => right_flanking || chars.get(start.wrapping_sub(1)).copied() == Some('~'),
        '~' => right_flanking,
        _ => false,
    }
}

fn delimiter_flanking(chars: &[char], start: usize, run_len: usize) -> (bool, bool, bool, bool) {
    let prev = if start == 0 {
        None
    } else {
        chars.get(start - 1).copied()
    };
    let next = chars.get(start + run_len).copied();

    let prev_is_whitespace = prev.is_none_or(char::is_whitespace);
    let next_is_whitespace = next.is_none_or(char::is_whitespace);
    let prev_is_punct = prev.is_some_and(is_markdown_punctuation);
    let next_is_punct = next.is_some_and(is_markdown_punctuation);

    let left_flanking =
        !next_is_whitespace && (!next_is_punct || prev_is_whitespace || prev_is_punct);
    let right_flanking =
        !prev_is_whitespace && (!prev_is_punct || next_is_whitespace || next_is_punct);

    (left_flanking, right_flanking, prev_is_punct, next_is_punct)
}

fn is_markdown_punctuation(ch: char) -> bool {
    !ch.is_alphanumeric() && !ch.is_whitespace()
}

fn find_single_char(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i < chars.len() {
        if chars[i] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_matching_bracket(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    if start >= chars.len() || chars[start] != open {
        return None;
    }

    let mut i = start;
    let mut depth = 0usize;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }

        if ch == '`' {
            let run_len = count_consecutive(chars, i, '`');
            if let Some(end) = find_code_span_end(chars, i) {
                i = end + 1;
                continue;
            }
            i += run_len.max(1);
            continue;
        }

        if ch == '<' {
            if let Some((_, _, end)) = parse_autolink_like(chars, i) {
                i = end + 1;
                continue;
            }
            if let Some(end) = parse_raw_html(chars, i) {
                i = end;
                continue;
            }
        }

        if ch == open {
            depth += 1;
        } else if ch == close {
            if depth == 0 {
                return None;
            }
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }

        i += 1;
    }

    None
}

fn parse_code_span(chars: &[char], start: usize) -> Option<(String, usize)> {
    let open_len = count_consecutive(chars, start, '`');
    if open_len == 0 {
        return None;
    }

    let content_start = start + open_len;
    let close = find_code_span_end(chars, start)?;
    let close_start = close + 1 - open_len;
    let raw_code: String = chars_to_string(&chars[content_start..close_start]);

    let code = normalize_code_content(&raw_code);
    Some((code, close))
}

fn count_consecutive(chars: &[char], start: usize, marker: char) -> usize {
    let mut i = start;
    while i < chars.len() && chars[i] == marker {
        i += 1;
    }
    i.saturating_sub(start)
}

fn chars_to_string(chars: &[char]) -> String {
    let mut out = String::with_capacity(chars.len());
    out.extend(chars.iter().copied());
    out
}

fn find_code_span_end(chars: &[char], start: usize) -> Option<usize> {
    let open_len = count_consecutive(chars, start, '`');
    if open_len == 0 || start + open_len > chars.len() {
        return None;
    }

    let mut i = start + open_len;
    while i + open_len <= chars.len() {
        if chars[i..i + open_len].iter().all(|ch| *ch == '`') {
            let prev_ok = i == 0 || chars[i - 1] != '`';
            let next_ok = i + open_len >= chars.len() || chars[i + open_len] != '`';
            if prev_ok && next_ok {
                return Some(i + open_len - 1);
            }
        }
        i += 1;
    }

    None
}
fn normalize_code_content(raw: &str) -> String {
    let mut code = if raw.contains('\n') {
        let mut normalized = String::with_capacity(raw.len());
        for ch in raw.chars() {
            normalized.push(if ch == '\n' { ' ' } else { ch });
        }
        normalized
    } else {
        raw.to_string()
    };

    if code.starts_with(' ') && code.ends_with(' ') && code.len() > 1 {
        code.drain(..1);
        code.pop();
    }
    code
}

fn normalize_inline_plain_text(raw: String) -> String {
    if !raw.contains(") ") || !raw.contains('(') {
        return raw;
    }

    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != ')' {
            out.push(ch);
            continue;
        }

        let mut spaces = 0usize;
        while chars.peek() == Some(&' ') {
            chars.next();
            spaces += 1;
        }

        if spaces > 0 && chars.peek() == Some(&'(') {
            out.push(')');
            out.push(' ');
            continue;
        }

        out.push(')');
        for _ in 0..spaces {
            out.push(' ');
        }
    }

    out
}

fn parse_autolink_like(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if start + 1 >= chars.len() || chars[start] != '<' {
        return None;
    }
    let close = find_single_char(chars, start + 1, '>')?;
    if close <= start + 1 {
        return None;
    }

    let inner = chars_to_string(&chars[start + 1..close]);
    let trimmed = inner.trim();
    if trimmed.is_empty()
        || inner != trimmed
        || trimmed.contains(' ')
        || trimmed.contains('\n')
        || trimmed.contains('\r')
    {
        return None;
    }

    let href = if is_autolink_uri(trimmed) {
        normalize_autolink_destination(trimmed)?
    } else if is_autolink_email(trimmed) {
        format!("mailto:{trimmed}")
    } else {
        return None;
    };

    Some((href, trimmed.to_string(), close))
}

fn normalize_autolink_destination(raw: &str) -> Option<String> {
    if raw.is_empty() {
        return Some(String::new());
    }

    Some(percent_encode_autolink_destination(&decode_html_entities(
        raw,
    )))
}

fn percent_encode_autolink_destination(raw: &str) -> String {
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
            '`' => out.push_str("%60"),
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

fn parse_quoted_autolink_like(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if start + 1 >= chars.len() {
        return None;
    }
    let quote = chars[start];
    if quote != '"' && quote != '\'' {
        return None;
    }

    let mut close = chars.len();
    while close > start + 1 {
        if chars[close - 1] == quote {
            let inner: String = chars_to_string(&chars[start + 1..close - 1]);
            if inner.is_empty()
                || inner.contains(' ')
                || inner.contains('\n')
                || inner.contains('\r')
            {
                close -= 1;
                continue;
            }
            if is_autolink_uri(&inner) || is_autolink_email(&inner) {
                let href = if is_autolink_email(&inner) {
                    format!("mailto:{inner}")
                } else {
                    inner.clone()
                };
                return Some((href, inner, close - 1));
            }
        }
        close -= 1;
    }

    None
}

fn is_autolink_uri(raw: &str) -> bool {
    let Some((scheme, rest)) = raw.split_once(':') else {
        return false;
    };
    if scheme.len() < 2 || scheme.len() > 32 || rest.is_empty() {
        return false;
    }
    let mut scheme_chars = scheme.chars();
    let Some(first) = scheme_chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    if !scheme_chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return false;
    }
    !rest
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '<' | '>'))
}

fn is_autolink_email(raw: &str) -> bool {
    let at = match raw.find('@') {
        Some(v) => v,
        None => return false,
    };
    if at == 0 || at + 1 >= raw.len() {
        return false;
    }
    let (local, domain) = raw.split_at(at);
    let domain = &domain[1..];
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if domain.ends_with('.') || domain.starts_with('.') {
        return false;
    }
    let domain_parts: Vec<&str> = domain.split('.').collect();
    if domain_parts.len() < 2 {
        return false;
    }
    if domain_parts.iter().any(|part| part.is_empty()) {
        return false;
    }
    if !local
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '+' || c == '-')
    {
        return false;
    }
    if !domain_parts
        .iter()
        .all(|part| part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    {
        return false;
    }
    true
}

fn parse_link_like(
    chars: &[char],
    start: usize,
    pedantic: bool,
) -> Option<(String, usize, String, Option<String>)> {
    let close_label = find_matching_bracket(chars, start, '[', ']')?;
    if close_label + 1 >= chars.len() || chars[close_label + 1] != '(' {
        return None;
    }
    let (href, title, close_href) = parse_inline_link_target(chars, close_label + 2, pedantic)?;
    let label = chars_to_string(&chars[start + 1..close_label]);
    Some((href, close_href, label, title))
}

fn parse_reference_link(
    chars: &[char],
    start: usize,
    gfm: bool,
    pedantic: bool,
    refs: Option<&HashMap<String, ReferenceDefinition>>,
) -> Option<(usize, String, Option<String>, String)> {
    let _ = gfm;
    if start >= chars.len() || chars[start] != '[' {
        return None;
    }
    let close_label = find_matching_bracket(chars, start, '[', ']')?;
    let label = chars_to_string(&chars[start + 1..close_label]);
    let candidate_ref = label.trim();

    let mut next = close_label + 1;
    if pedantic {
        while next < chars.len() && matches!(chars[next], ' ' | '\t' | '\n' | '\r') {
            next += 1;
        }
    }
    if next < chars.len() && chars[next] == '[' {
        let label_start = next + 1;
        if label_start > chars.len() {
            return None;
        }

        let (ref_label, close_ref) = if label_start < chars.len() && chars[label_start] == ']' {
            (candidate_ref.to_string(), label_start)
        } else {
            let close = find_matching_bracket(chars, next, '[', ']')?;
            (chars_to_string(&chars[label_start..close]), close)
        };
        let normalized = try_normalize_reference_label(&ref_label)?;
        if let Some(def) =
            refs.and_then(|m: &HashMap<String, ReferenceDefinition>| m.get(&normalized))
        {
            return Some((close_ref, def.href.clone(), def.title.clone(), label));
        }
        return None;
    }

    let normalized = normalize_reference_label(candidate_ref);
    if let Some(def) = refs.and_then(|m: &HashMap<String, ReferenceDefinition>| m.get(&normalized))
    {
        Some((close_label, def.href.clone(), def.title.clone(), label))
    } else {
        None
    }
}

fn parse_reference_image(
    chars: &[char],
    start: usize,
    gfm: bool,
    pedantic: bool,
    refs: Option<&HashMap<String, ReferenceDefinition>>,
) -> Option<(usize, String, Option<String>, String)> {
    let _ = gfm;
    if !is_unescaped_image_marker(chars, start) {
        return None;
    }

    let close_alt = find_matching_bracket(chars, start, '[', ']')?;
    let alt = chars_to_string(&chars[start + 1..close_alt]);
    let candidate_ref = alt.trim();

    let mut next = close_alt + 1;
    if pedantic {
        while next < chars.len() && matches!(chars[next], ' ' | '\t' | '\n' | '\r') {
            next += 1;
        }
    }
    if next < chars.len() && chars[next] == '[' {
        let label_start = next + 1;
        let (ref_label, close_ref) = if label_start < chars.len() && chars[label_start] == ']' {
            (candidate_ref.to_string(), label_start)
        } else {
            let close = find_matching_bracket(chars, next, '[', ']')?;
            (chars_to_string(&chars[label_start..close]), close)
        };
        let normalized = try_normalize_reference_label(&ref_label)?;
        if let Some(def) = refs.and_then(|m| m.get(&normalized)) {
            return Some((close_ref, def.href.clone(), def.title.clone(), alt));
        }
        return None;
    }

    let normalized = normalize_reference_label(candidate_ref);
    refs.and_then(|m| m.get(&normalized))
        .map(|def| (close_alt, def.href.clone(), def.title.clone(), alt))
}

fn is_escapable(ch: char) -> bool {
    ch.is_ascii_punctuation()
}

fn parse_image_like(
    chars: &[char],
    start: usize,
    pedantic: bool,
) -> Option<(String, String, Option<String>, usize)> {
    if !is_unescaped_image_marker(chars, start) {
        return None;
    }
    let close_alt = find_matching_bracket(chars, start, '[', ']')?;
    if close_alt + 1 >= chars.len() || chars[close_alt + 1] != '(' {
        return None;
    }
    let (src, title, close_src) = parse_inline_link_target(chars, close_alt + 2, pedantic)?;
    let alt = chars_to_string(&chars[start + 1..close_alt]);
    Some((src, alt, title, close_src))
}

fn parse_inline_link_target(
    chars: &[char],
    start: usize,
    pedantic: bool,
) -> Option<(String, Option<String>, usize)> {
    let mut i = start;
    while i < chars.len() && is_markdown_whitespace(chars[i]) {
        i += 1;
    }

    if i < chars.len() && chars[i] == ')' {
        return Some((String::new(), None, i));
    }

    if pedantic && i < chars.len() && chars[i] != '<' {
        if let Some(parsed) = parse_pedantic_bare_link_target(chars, i) {
            return Some(parsed);
        }
    }

    let (href, after_dest) = if i < chars.len() && chars[i] == '<' {
        parse_angle_link_destination(chars, i, pedantic)?
    } else {
        parse_bare_link_destination(chars, i)?
    };

    let mut j = after_dest;
    while j < chars.len() && is_markdown_whitespace(chars[j]) {
        j += 1;
    }
    let had_separator = j > after_dest;

    if j >= chars.len() {
        return None;
    }
    if chars[j] == ')' {
        return Some((href, None, j));
    }
    if !had_separator {
        return None;
    }

    let (title, after_title) = parse_link_title_chars_mode(chars, j, pedantic)?;
    let mut k = after_title;
    while k < chars.len() && is_markdown_whitespace(chars[k]) {
        k += 1;
    }
    if k >= chars.len() || chars[k] != ')' {
        return None;
    }

    Some((href, Some(title), k))
}

fn parse_pedantic_bare_link_target(
    chars: &[char],
    start: usize,
) -> Option<(String, Option<String>, usize)> {
    let close = find_pedantic_link_target_end(chars, start)?;
    let inner = chars_to_string(&chars[start..close]);
    let trimmed = inner.trim_start();
    if trimmed.is_empty() {
        return Some((String::new(), None, close));
    }

    if let Some((dest, title)) = split_pedantic_destination_and_title(trimmed) {
        return Some((normalize_reference_destination(&dest)?, Some(title), close));
    }

    Some((
        normalize_reference_destination(trimmed.trim_end())?,
        None,
        close,
    ))
}

fn find_pedantic_link_target_end(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if ch == ')' {
            return Some(i);
        }
        if ch == '\n' || ch == '\r' {
            return None;
        }
        i += 1;
    }

    None
}

fn split_pedantic_destination_and_title(raw: &str) -> Option<(String, String)> {
    for (idx, ch) in raw.char_indices() {
        if !is_markdown_whitespace(ch) {
            continue;
        }
        let dest = raw[..idx].trim_end();
        if dest.is_empty() {
            continue;
        }
        let title_raw = raw[idx..].trim_start();
        if let Some((title, consumed)) = parse_link_title_str(title_raw, true) {
            if title_raw[consumed..].trim().is_empty() {
                return Some((dest.to_string(), title));
            }
        }
    }

    None
}

fn parse_angle_link_destination(
    chars: &[char],
    start: usize,
    pedantic: bool,
) -> Option<(String, usize)> {
    if chars.get(start).copied() != Some('<') {
        return None;
    }

    let mut i = start + 1;
    let mut raw = String::new();
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' {
            let next = *chars.get(i + 1)?;
            if is_escapable(next) {
                raw.push(next);
                i += 2;
                continue;
            }
            raw.push('\\');
            i += 1;
            continue;
        }
        if ch == '>' {
            return Some((normalize_reference_destination(&raw)?, i + 1));
        }
        if ch == '\n' || ch == '\r' || ch == '<' {
            return None;
        }
        raw.push(ch);
        i += 1;
    }

    if !pedantic {
        return None;
    }

    let mut close = start + 1;
    while close < chars.len() && chars[close] != ')' {
        if chars[close] == '\n' || chars[close] == '\r' {
            return None;
        }
        close += 1;
    }
    if close >= chars.len() || chars[close] != ')' {
        return None;
    }

    let mut raw = chars_to_string(&chars[start + 1..close]);
    if raw.ends_with('>') {
        raw.pop();
    }
    Some((normalize_reference_destination(&raw)?, close))
}

fn parse_bare_link_destination(chars: &[char], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    let mut depth = 0usize;
    let mut escaped = false;

    while i < chars.len() {
        let ch = chars[i];
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if is_markdown_whitespace(ch) {
            break;
        }
        if ch == '(' {
            depth += 1;
            i += 1;
            continue;
        }
        if ch == ')' {
            if depth == 0 {
                break;
            }
            depth -= 1;
            i += 1;
            continue;
        }
        i += 1;
    }

    if i == start {
        return None;
    }

    let raw = chars_to_string(&chars[start..i]);
    Some((normalize_reference_destination(&raw)?, i))
}

fn parse_link_title_chars_mode(
    chars: &[char],
    start: usize,
    pedantic: bool,
) -> Option<(String, usize)> {
    let quote = *chars.get(start)?;
    let close = match quote {
        '"' | '\'' => quote,
        '(' => ')',
        _ => return None,
    };

    let end = if pedantic && matches!(quote, '"' | '\'') {
        find_last_unescaped_title_close_chars(chars, start, close)?
    } else {
        find_first_unescaped_title_close_chars(chars, start, close)?
    };

    let title = decode_html_entities(&unescape_inline(&chars_to_string(&chars[start + 1..end])));
    Some((title, end + 1))
}

fn parse_link_title_str(raw: &str, pedantic: bool) -> Option<(String, usize)> {
    let quote = raw.chars().next()?;
    let close = match quote {
        '"' | '\'' => quote,
        '(' => ')',
        _ => return None,
    };

    let end = if pedantic && matches!(quote, '"' | '\'') {
        find_last_unescaped_title_close(raw, close)?
    } else {
        find_first_unescaped_title_close(raw, close)?
    };

    let content_start = quote.len_utf8();
    let title = decode_html_entities(&unescape_inline(&raw[content_start..end]));
    let consumed = end + close.len_utf8();
    Some((title, consumed))
}

fn find_first_unescaped_title_close(raw: &str, close: char) -> Option<usize> {
    let mut escaped = false;
    for (idx, ch) in raw.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == close {
            return Some(idx);
        }
    }
    None
}

fn find_last_unescaped_title_close(raw: &str, close: char) -> Option<usize> {
    let mut candidate = None;
    let mut escaped = false;
    for (idx, ch) in raw.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == close {
            candidate = Some(idx);
        }
    }
    candidate
}

fn find_first_unescaped_title_close_chars(
    chars: &[char],
    start: usize,
    close: char,
) -> Option<usize> {
    let mut escaped = false;
    let mut idx = start + 1;
    while idx < chars.len() {
        let ch = chars[idx];
        if escaped {
            escaped = false;
            idx += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            idx += 1;
            continue;
        }
        if ch == close {
            return Some(idx);
        }
        idx += 1;
    }
    None
}

fn find_last_unescaped_title_close_chars(
    chars: &[char],
    start: usize,
    close: char,
) -> Option<usize> {
    let mut candidate = None;
    let mut escaped = false;
    let mut idx = start + 1;
    while idx < chars.len() {
        let ch = chars[idx];
        if escaped {
            escaped = false;
            idx += 1;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            idx += 1;
            continue;
        }
        if ch == close {
            candidate = Some(idx);
        }
        idx += 1;
    }
    candidate
}

fn parse_html_entity_chars(chars: &[char], start: usize) -> Option<(String, usize)> {
    if chars.get(start).copied() != Some('&') {
        return None;
    }

    let mut end = start + 1;
    while end < chars.len() {
        match chars[end] {
            ';' => {
                let raw = chars_to_string(&chars[start..=end]);
                let (decoded, _) = parse_html_entity(&raw)?;
                return Some((decoded, end - start + 1));
            }
            '&' | ' ' | '\t' | '\n' | '\r' => return None,
            _ => end += 1,
        }
    }

    None
}

fn is_unescaped_image_marker(chars: &[char], start: usize) -> bool {
    start > 0
        && chars[start] == '['
        && chars[start - 1] == '!'
        && !is_escaped_char(chars, start - 1)
}

fn is_escaped_char(chars: &[char], index: usize) -> bool {
    if index == 0 {
        return false;
    }

    let mut backslashes = 0usize;
    let mut i = index;
    while i > 0 && chars[i - 1] == '\\' {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1
}

fn is_markdown_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

fn unescape_inline(raw: &str) -> String {
    if raw.is_empty() || !raw.contains('\\') {
        return raw.to_string();
    }

    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            } else {
                out.push('\\');
            }
            continue;
        }
        out.push(ch);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_quoted_title() {
        let chars: Vec<char> = "logo.png 'Markec logo')".chars().collect();
        let Some((href, title, close)) = parse_inline_link_target(&chars, 0, false) else {
            panic!("expected parsed href/title");
        };
        assert_eq!(href, "logo.png");
        assert_eq!(title, Some("Markec logo".to_string()));
        assert_eq!(close, chars.len() - 1);
    }

    #[test]
    fn parses_image_inline_title() {
        let nodes = parse_inline("![logo](logo.png 'Markec logo')", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Image { src, title, .. } => {
                assert_eq!(src, "logo.png");
                assert_eq!(title.as_deref(), Some("Markec logo"));
            }
            _ => panic!("expected image node"),
        }
    }

    #[test]
    fn parses_autolink_like_url() {
        let nodes = parse_inline("<http://example.com>", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Link { href, label, title } => {
                assert_eq!(href, "http://example.com");
                assert_eq!(title, &None);
                assert_eq!(label.len(), 1);
                match &label[0] {
                    Inline::Text(text) => assert_eq!(text, "http://example.com"),
                    _ => panic!("expected text label"),
                }
            }
            _ => panic!("expected autolink node"),
        }
    }

    #[test]
    fn parses_autolink_like_email() {
        let nodes = parse_inline("<hello@example.com>", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Link { href, label, .. } => {
                assert_eq!(href, "mailto:hello@example.com");
                assert_eq!(label.len(), 1);
                match &label[0] {
                    Inline::Text(text) => assert_eq!(text, "hello@example.com"),
                    _ => panic!("expected text label"),
                }
            }
            _ => panic!("expected autolink node"),
        }
    }

    #[test]
    fn parses_variable_length_code_span() {
        let nodes = parse_inline("``hello world``", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Code(code) => assert_eq!(code, "hello world"),
            _ => panic!("expected code node"),
        }

        let nodes = parse_inline("```hello```", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Code(code) => assert_eq!(code, "hello"),
            _ => panic!("expected code node"),
        }

        let nodes = parse_inline("``foo `bar` baz``", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Code(code) => assert_eq!(code, "foo `bar` baz"),
            _ => panic!("expected code node"),
        }
    }

    #[test]
    fn parses_backtick_precedence_samples() {
        let nodes = parse_inline("**This should be bold ``**`", true);
        assert_eq!(nodes.len(), 2);
        assert!(matches!(nodes[0], Inline::Strong(_)));
        assert!(matches!(nodes[1], Inline::Text(ref t) if t == "`"));

        let nodes = parse_inline("**This should be bold `**`", true);
        assert!(!nodes.is_empty());

        let nodes = parse_inline("**You might think this should be bold, but: `**`", true);
        assert!(!nodes.iter().any(|node| matches!(node, Inline::Strong(_))));
        assert!(!nodes.is_empty());
        assert!(nodes.iter().any(|node| matches!(node, Inline::Code(_))));

        let nodes = parse_inline("**This should be bold `**``", true);
        assert!(!nodes.is_empty());
    }

    #[test]
    fn parses_link_like_nested_parentheses_and_escapes() {
        let nodes = parse_inline("[link](foo(bar())", true);
        assert!(!matches!(nodes.first(), Some(Inline::Link { .. })));
        assert!(!nodes.is_empty());

        let nodes = parse_inline("[link](foo\\(bar())", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Link { href, .. } => assert_eq!(href, "foo(bar()"),
            _ => panic!("expected link node"),
        }

        let nodes = parse_inline("[link](foo(bar\\\\())", true);
        assert!(!matches!(nodes.first(), Some(Inline::Link { .. })));
        assert!(!nodes.is_empty());
    }

    #[test]
    fn parses_nested_square_link() {
        let nodes = parse_inline("[the `]` character](/url)", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Link { href, label, .. } => {
                assert_eq!(href, "/url");
                assert_eq!(label.len(), 3);
                assert!(matches!(label[0], Inline::Text(ref t) if t == "the "));
                assert!(matches!(label[1], Inline::Code(ref c) if c == "]"));
                assert!(matches!(label[2], Inline::Text(ref t) if t == " character"));
            }
            _ => panic!("expected link node"),
        }
    }

    #[test]
    fn parses_links_with_paren_and_spacing_variants() {
        let nodes = parse_inline("( [one](http://example.com/1) )", true);
        assert_eq!(nodes.len(), 3);
        assert!(matches!(&nodes[1], Inline::Link { href, .. } if href == "http://example.com/1"));
        let nodes = parse_inline("( [one](http://example.com/1 \"a\") )", true);
        assert_eq!(nodes.len(), 3);
        assert!(matches!(&nodes[1], Inline::Link { title, .. } if title == &Some("a".to_string())));
    }

    #[test]
    fn parses_raw_html_inline_node() {
        let nodes = parse_inline("<a href=\"https://example.com\">x</a>", false);
        assert_eq!(nodes.len(), 3);
        assert!(matches!(nodes[0], Inline::RawHtml(_)));
        assert!(matches!(nodes[1], Inline::Text(ref t) if t == "x"));
        assert!(matches!(nodes[2], Inline::RawHtml(_)));
    }

    #[test]
    fn parses_raw_html_comment_nodes() {
        let nodes = parse_inline("<!-- comment -->", false);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], Inline::RawHtml(_)));

        let nodes = parse_inline("<!--> a comment -->", false);
        assert_eq!(nodes.len(), 1);
        assert!(matches!(nodes[0], Inline::RawHtml(_)));
    }

    #[test]
    fn parses_reference_links_with_whitespace_between_labels() {
        let mut refs = std::collections::HashMap::new();
        refs.insert(
            "1".to_string(),
            ReferenceDefinition {
                href: "/url/".to_string(),
                title: Some("Title".to_string()),
            },
        );
        refs.insert(
            "this".to_string(),
            ReferenceDefinition {
                href: "foo".to_string(),
                title: None,
            },
        );

        let nodes = parse_inline_with_refs("Foo [bar] [1].", true, false, Some(&refs));
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node, Inline::Link { href, .. } if href == "/url/"))
        );

        let nodes = parse_inline_with_refs("And [this] [].", true, false, Some(&refs));
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node, Inline::Link { href, .. } if href == "foo"))
        );
    }

    #[test]
    fn rejects_angle_link_destination_with_escaped_close_bracket() {
        let nodes = parse_inline("[URL](<test\\>)", true);
        assert!(!matches!(nodes.first(), Some(Inline::Link { .. })));
    }

    #[test]
    fn parses_link_destinations_with_entities_and_backslashes() {
        let nodes = parse_inline("[link](foo%20b&auml;)", true);
        assert!(matches!(&nodes[0], Inline::Link { href, .. } if href == "foo%20b%C3%A4"));

        let nodes = parse_inline("[link](foo\\bar)", true);
        assert!(matches!(&nodes[0], Inline::Link { href, .. } if href == "foo%5Cbar"));
    }

    #[test]
    fn keeps_non_ascii_space_inside_link_destination() {
        let nodes = parse_inline("[link](/url\u{00A0}\"title\")", true);
        assert!(
            matches!(&nodes[0], Inline::Link { href, title, .. } if href == "/url%C2%A0%22title%22" && title.is_none())
        );
    }

    #[test]
    fn parses_emphasis_wrapping_link_with_underscore_in_destination() {
        let nodes = parse_inline("_[test](https://example.com?link=with_(underscore))_", true);
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            Inline::Em(children) => {
                assert_eq!(children.len(), 1);
                assert!(matches!(children[0], Inline::Link { .. }));
            }
            _ => panic!("expected em node"),
        }
    }
}
