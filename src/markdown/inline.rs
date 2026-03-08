use std::collections::HashMap;

use crate::markdown::{
    ast::inline::Inline,
    block::{
        ReferenceDefinition, decode_html_entities, is_valid_reference_label,
        normalize_reference_destination, normalize_reference_label, parse_html_entity,
    },
};

pub(crate) struct InlineParser<'a> {
    input: &'a str,
    gfm: bool,
    pedantic: bool,
    refs: Option<&'a HashMap<String, ReferenceDefinition>>,
}

impl<'a> InlineParser<'a> {
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

pub(crate) fn parse_inline(input: &str, gfm: bool) -> Vec<Inline> {
    InlineParser::new(input, gfm, false).parse()
}

pub(crate) fn parse_inline_with_refs(
    input: &str,
    gfm: bool,
    pedantic: bool,
    refs: Option<&HashMap<String, ReferenceDefinition>>,
) -> Vec<Inline> {
    let mut out: Vec<InlinePart> = Vec::new();
    let chars: Vec<char> = input.chars().collect();
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
                push_inline_part(&mut out, InlinePart::Node(Inline::Text("\n".to_string())));
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
                    push_inline_part(
                        &mut out,
                        InlinePart::Node(Inline::Text(chars[i + 1].to_string())),
                    );
                    i += 2;
                    continue;
                }
            }
            push_inline_part(&mut out, InlinePart::Node(Inline::Text("\\".to_string())));
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
                    InlinePart::Node(Inline::RawHtml(chars[i..close].iter().collect())),
                );
                i = close;
                continue;
            }
        }

        if let Some((href, label, close)) = parse_quoted_autolink_like(&chars, i) {
            push_inline_part(
                &mut out,
                InlinePart::Node(Inline::Text(chars[i].to_string())),
            );
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
            push_inline_part(&mut out, delimiter);
            i += run_len;
            continue;
        }

        if chars[i] == '`' {
            let open_len = count_consecutive(&chars, i, '`');
            if open_len > 1 && parse_code_span(&chars, i).is_none() {
                push_inline_part(
                    &mut out,
                    InlinePart::Node(Inline::Text(chars[i..i + open_len].iter().collect())),
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
                let parsed_alt = parse_inline_with_refs(&alt, gfm, pedantic, refs);
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
                let parsed_alt = parse_inline_with_refs(&alt, gfm, pedantic, refs);
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
                let parsed_label = parse_inline_with_refs(&label, gfm, pedantic, refs);
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
                let parsed_label = parse_inline_with_refs(&label, gfm, pedantic, refs);
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
            let tail = chars[i..].iter().collect::<String>();
            if let Some((decoded, consumed)) = parse_html_entity(&tail) {
                push_inline_part(&mut out, InlinePart::Node(Inline::Text(decoded)));
                i += tail[..consumed].chars().count();
                continue;
            }
        }

        let mut plain = String::new();
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
            InlinePart::Node(Inline::Text(normalize_inline_plain_text(&plain))),
        );
    }

    resolve_inline_parts(out)
}

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

fn parse_delimiter_run(chars: &[char], start: usize, gfm: bool) -> Option<(InlinePart, usize)> {
    let marker = chars.get(start).copied()?;
    if marker != '*' && marker != '_' && !(gfm && marker == '~') {
        return None;
    }

    let run_len = count_consecutive(chars, start, marker);
    if marker == '~' && run_len > 2 {
        return Some((
            InlinePart::Node(Inline::Text(chars[start..start + run_len].iter().collect())),
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
        InlinePart::Node(Inline::Text(chars[start..start + run_len].iter().collect()))
    };

    Some((part, run_len))
}

fn resolve_inline_parts(parts: Vec<InlinePart>) -> Vec<Inline> {
    let resolved = resolve_delimiter_runs(parts);
    let mut out = Vec::new();
    for part in resolved {
        match part {
            InlinePart::Node(node) => push_inline_node(&mut out, node),
            InlinePart::Delimiter { marker, len, .. } => {
                push_inline_node(&mut out, Inline::Text(marker.to_string().repeat(len)));
            }
        }
    }
    out
}

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

fn resolve_delimiter_runs(mut parts: Vec<InlinePart>) -> Vec<InlinePart> {
    loop {
        let mut changed = false;

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

            let (opener_len, opener_original_len) = match &parts[opener_idx] {
                InlinePart::Delimiter {
                    len, original_len, ..
                } => (*len, *original_len),
                InlinePart::Node(_) => unreachable!("matched opener must be delimiter"),
            };
            let (opener_can_open, opener_can_close) = match &parts[opener_idx] {
                InlinePart::Delimiter {
                    can_open,
                    can_close,
                    ..
                } => (*can_open, *can_close),
                InlinePart::Node(_) => unreachable!("matched opener must be delimiter"),
            };

            let inner = resolve_inline_parts(parts[opener_idx + 1..closer_idx].to_vec());
            let wrapped = match (*marker, use_len) {
                ('~', _) => Inline::Del(inner),
                (_, 2) => Inline::Strong(inner),
                _ => Inline::Em(inner),
            };

            let mut next = Vec::with_capacity(parts.len());
            next.extend(parts[..opener_idx].iter().cloned());

            if opener_len > use_len {
                next.push(InlinePart::Delimiter {
                    marker: *marker,
                    len: opener_len - use_len,
                    original_len: opener_original_len,
                    can_open: opener_can_open,
                    can_close: opener_can_close,
                });
            }

            next.push(InlinePart::Node(wrapped));

            if *closer_len > use_len {
                let closer_can_close = *can_close;
                next.push(InlinePart::Delimiter {
                    marker: *marker,
                    len: *closer_len - use_len,
                    original_len: *closer_original_len,
                    can_open: *closer_can_open,
                    can_close: closer_can_close,
                });
            }

            next.extend(parts[closer_idx + 1..].iter().cloned());
            parts = next;
            changed = true;
            break;
        }

        if !changed {
            return parts;
        }
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
    let expected: Vec<char> = s.chars().collect();
    if i + expected.len() > chars.len() {
        return false;
    }
    expected
        .iter()
        .enumerate()
        .all(|(offset, expected_c)| chars[i + offset] == *expected_c)
}

fn find_single_delimiter(chars: &[char], start: usize, marker: char) -> Option<usize> {
    find_delimiter(chars, start, &[marker])
}

fn find_delimiter(chars: &[char], start: usize, marker: &[char]) -> Option<usize> {
    if marker.is_empty() {
        return None;
    }

    let mut i = start;
    while i + marker.len() <= chars.len() {
        if chars[i] == '[' {
            if let Some((_, close_link, _, _)) = parse_link_like(chars, i, false) {
                i = close_link + 1;
                continue;
            }
            if let Some(close_label) = find_matching_bracket(chars, i, '[', ']') {
                i = close_label + 1;
                continue;
            }
        }
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((_, _, _, close_src)) = parse_image_like(chars, i + 1, false) {
                i = close_src + 1;
                continue;
            }
        }

        if is_slice_match(chars, i, marker) {
            let run_len = count_consecutive(chars, i, marker[0]);
            if !is_delimiter_inside_code(chars, start, i)
                && run_len >= marker.len()
                && delimiter_run_can_close(chars, i, run_len, marker[0])
            {
                return Some(i);
            }
        }

        if chars[i] == '`' {
            let run_len = count_consecutive(chars, i, '`');
            if marker.len() > 1 {
                i += 1;
                continue;
            }

            if let Some(end) = find_code_span_end(chars, i) {
                i = end + 1;
                continue;
            }

            i += run_len.max(1);
            continue;
        }

        i += 1;
    }
    None
}

fn is_delimiter_inside_code(chars: &[char], scan_start: usize, candidate: usize) -> bool {
    if scan_start >= candidate {
        return false;
    }

    let mut i = scan_start;
    while i < candidate {
        if chars[i] == '`' {
            let run_len = count_consecutive(chars, i, '`');

            if let Some(end) = find_code_span_end(chars, i) {
                if candidate > i && candidate <= end {
                    return true;
                }
                i = end + 1;
                continue;
            }

            i += run_len.max(1);
            continue;
        }
        i += 1;
    }

    false
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
    let raw_code: String = chars[content_start..close_start].iter().collect();

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

fn find_code_span_end(chars: &[char], start: usize) -> Option<usize> {
    let open_len = count_consecutive(chars, start, '`');
    if open_len == 0 || start + open_len > chars.len() {
        return None;
    }

    let mut i = start + open_len;
    while i + open_len <= chars.len() {
        if is_slice_match(chars, i, &vec!['`'; open_len]) {
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
    let mut code = raw.replace('\n', " ");
    if code.starts_with(' ') && code.ends_with(' ') && code.len() > 1 {
        code = code[1..code.len() - 1].to_string();
    }
    code
}

fn normalize_inline_plain_text(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == ')' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] == ' ' {
                j += 1;
            }

            if j > i + 1 && j < chars.len() && chars[j] == '(' {
                out.push(')');
                out.push(' ');
                i = j;
                continue;
            }
        }

        out.push(chars[i]);
        i += 1;
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

    let inner = chars[start + 1..close].iter().collect::<String>();
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
            let inner: String = chars[start + 1..close - 1].iter().collect();
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

fn is_slice_match(chars: &[char], i: usize, marker: &[char]) -> bool {
    if i + marker.len() > chars.len() {
        return false;
    }
    marker
        .iter()
        .enumerate()
        .all(|(offset, c)| chars[i + offset] == *c)
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
    let label = chars[start + 1..close_label].iter().collect::<String>();
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
    let label = chars[start + 1..close_label].iter().collect::<String>();
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
            (chars[label_start..close].iter().collect::<String>(), close)
        };
        if !is_valid_reference_label(&ref_label) {
            return None;
        }

        let normalized = normalize_reference_label(&ref_label);
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
    let alt = chars[start + 1..close_alt].iter().collect::<String>();
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
            (chars[label_start..close].iter().collect::<String>(), close)
        };
        if !is_valid_reference_label(&ref_label) {
            return None;
        }

        let normalized = normalize_reference_label(&ref_label);
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
    let alt = chars[start + 1..close_alt].iter().collect::<String>();
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
    let inner = chars[start..close].iter().collect::<String>();
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

    let mut raw = chars[start + 1..close].iter().collect::<String>();
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

    let raw = chars[start..i].iter().collect::<String>();
    Some((normalize_reference_destination(&raw)?, i))
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

fn parse_link_title_chars(chars: &[char], start: usize) -> Option<(String, usize)> {
    parse_link_title_chars_mode(chars, start, false)
}

fn parse_link_title_chars_mode(
    chars: &[char],
    start: usize,
    pedantic: bool,
) -> Option<(String, usize)> {
    let _ = *chars.get(start)?;
    let raw = chars[start..].iter().collect::<String>();
    let (title, consumed) = parse_link_title_str(&raw, pedantic)?;
    let consumed_chars = raw[..consumed].chars().count();
    Some((title, start + consumed_chars))
}

fn parse_link_title_str(raw: &str, pedantic: bool) -> Option<(String, usize)> {
    let chars = raw.chars().collect::<Vec<_>>();
    let quote = *chars.first()?;
    let close = match quote {
        '"' | '\'' => quote,
        '(' => ')',
        _ => return None,
    };

    let end = if pedantic && matches!(quote, '"' | '\'') {
        find_last_unescaped_title_close(&chars, close)?
    } else {
        find_first_unescaped_title_close(&chars, close)?
    };

    let title = decode_html_entities(&unescape_inline(chars[1..end].iter().collect::<String>()));
    let consumed = chars[..=end].iter().collect::<String>().len();
    Some((title, consumed))
}

fn find_first_unescaped_title_close(chars: &[char], close: char) -> Option<usize> {
    let mut i = 1usize;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        if chars[i] == close {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_last_unescaped_title_close(chars: &[char], close: char) -> Option<usize> {
    let mut candidate = None;
    let mut i = 1usize;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        if chars[i] == close {
            candidate = Some(i);
        }
        i += 1;
    }
    candidate
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

fn unescape_inline(raw: String) -> String {
    if raw.is_empty() {
        return raw;
    }

    let mut out = String::with_capacity(raw.len());
    let chars = raw.chars().collect::<Vec<_>>();
    let mut i = 0usize;

    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
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
