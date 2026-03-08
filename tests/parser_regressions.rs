use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use markrs::{RenderOptions, render_markdown_to_html};
use regex::Regex;
use serde_json::Value;

static INPUT_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<input([^>]*)>").expect("invalid regex"));
static WS_BETWEEN_TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">\s+<").expect("invalid regex"));
static XHTML_VOID_SLASH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(br|hr|img|input)([^>]*?)\s*/?\s*>").expect("invalid regex"));
static BR_FOLLOWED_BY_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<br>\s*\n+").expect("invalid regex"));
static TEXT_BEFORE_BLOCKQUOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^\s>])\s+<blockquote>").expect("invalid regex"));

fn compat_fixture_pair(name: &str) -> (String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let md = fs::read_to_string(
        root.join("third_party/marked/test/specs/new")
            .join(format!("{name}.md")),
    )
    .unwrap_or_else(|e| panic!("failed reading markdown fixture {name}: {e}"));
    let html = fs::read_to_string(
        root.join("third_party/marked/test/specs/new")
            .join(format!("{name}.html")),
    )
    .unwrap_or_else(|e| panic!("failed reading html fixture {name}: {e}"));
    (md, html)
}

fn compat_original_fixture_pair(name: &str) -> (String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let md = fs::read_to_string(
        root.join("third_party/marked/test/specs/original")
            .join(format!("{name}.md")),
    )
    .unwrap_or_else(|e| panic!("failed reading original markdown fixture {name}: {e}"));
    let html = fs::read_to_string(
        root.join("third_party/marked/test/specs/original")
            .join(format!("{name}.html")),
    )
    .unwrap_or_else(|e| panic!("failed reading original html fixture {name}: {e}"));
    (md, html)
}

fn commonmark_example_pair(example: u64) -> (String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw = fs::read_to_string(
        root.join("third_party/marked/test/specs/commonmark/commonmark.0.31.2.json"),
    )
    .unwrap_or_else(|e| panic!("failed reading commonmark fixture index: {e}"));
    let examples: Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid commonmark json: {e}"));
    let list = examples
        .as_array()
        .unwrap_or_else(|| panic!("commonmark fixture root is not an array"));
    let entry = list
        .iter()
        .find(|row| row.get("example").and_then(Value::as_u64) == Some(example))
        .unwrap_or_else(|| panic!("missing commonmark example {example}"));
    let markdown = entry
        .get("markdown")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing markdown for example {example}"))
        .to_string();
    let html = entry
        .get("html")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing html for example {example}"))
        .to_string();
    (markdown, html)
}

fn gfm_example_pair(example: u64) -> (String, String) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw = fs::read_to_string(root.join("third_party/marked/test/specs/gfm/gfm.0.29.json"))
        .unwrap_or_else(|e| panic!("failed reading gfm fixture index: {e}"));
    let examples: Value =
        serde_json::from_str(&raw).unwrap_or_else(|e| panic!("invalid gfm json: {e}"));
    let list = examples
        .as_array()
        .unwrap_or_else(|| panic!("gfm fixture root is not an array"));
    let idx = example
        .checked_sub(1)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| panic!("invalid gfm example index {example}"));
    let entry = list
        .get(idx)
        .unwrap_or_else(|| panic!("missing gfm example {example}"));
    let markdown = entry
        .get("markdown")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing markdown for gfm example {example}"))
        .to_string();
    let html = entry
        .get("html")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing html for gfm example {example}"))
        .to_string();
    (markdown, html)
}

fn render_compat_fixture(markdown: &str) -> String {
    let mut options = RenderOptions::default();
    let body = strip_marked_front_matter(markdown, &mut options);
    render_markdown_to_html(&body, options)
}

fn strip_marked_front_matter(markdown: &str, options: &mut RenderOptions) -> String {
    let Some(rest) = markdown.strip_prefix("---\n") else {
        return markdown.to_string();
    };
    let Some(end) = rest.find("\n---\n") else {
        return markdown.to_string();
    };

    let header = &rest[..end];
    for line in header.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim();
        if key == "gfm" {
            options.gfm = val == "true";
        }
        if key == "breaks" {
            options.breaks = val == "true";
        }
        if key == "pedantic" {
            options.pedantic = val == "true";
        }
    }

    rest[end + "\n---\n".len()..].to_string()
}

fn normalize_html(input: &str) -> String {
    let compact = WS_BETWEEN_TAGS.replace_all(input, "><");
    let normalized_void = XHTML_VOID_SLASH.replace_all(&compact, "<$1$2>");
    let normalized_break = BR_FOLLOWED_BY_NEWLINE.replace_all(&normalized_void, "<br>");
    let normalized_blockquote =
        TEXT_BEFORE_BLOCKQUOTE.replace_all(&normalized_break, "$1<blockquote>");
    let normalized_input = normalize_input_attrs(&normalized_blockquote);
    normalized_input
        .replace("\r\n", "\n")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x22;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&apos;", "'")
        .replace("&gt;", ">")
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn normalize_input_attrs(input: &str) -> String {
    INPUT_TAG
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let attrs = parse_attrs(caps.get(1).map(|m| m.as_str()).unwrap_or_default());
            if attrs.is_empty() {
                return "<input>".to_string();
            }

            let mut sorted = attrs;
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = String::from("<input");
            for (name, value) in sorted {
                out.push(' ');
                out.push_str(&name);
                if let Some(v) = value {
                    out.push_str("=\"");
                    out.push_str(&v);
                    out.push('"');
                }
            }
            out.push('>');
            out
        })
        .to_string()
}

fn parse_attrs(raw: &str) -> Vec<(String, Option<String>)> {
    let chars: Vec<char> = raw.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();

    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }

        let name_start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' {
            i += 1;
        }
        if name_start == i {
            i += 1;
            continue;
        }
        let name: String = chars[name_start..i].iter().collect();

        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }

        let mut value = None;
        if i < chars.len() && chars[i] == '=' {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
                let quote = chars[i];
                i += 1;
                let val_start = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                value = Some(chars[val_start..i].iter().collect());
                if i < chars.len() && chars[i] == quote {
                    i += 1;
                }
            } else {
                let val_start = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = Some(chars[val_start..i].iter().collect());
            }
        }

        out.push((name, value));
    }

    out
}

#[test]
fn compat_em_strong_complex_nesting_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("em_strong_complex_nesting");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_escape_newline_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("escape_newline");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_em_strong_orphaned_nesting_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("em_strong_orphaned_nesting");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_nested_em_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("nested_em");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_emoji_inline_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("emoji_inline");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_strikethrough_in_em_strong_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("strikethrough_in_em_strong");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_list_loose_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("list_loose");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_table_vs_setext_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("table_vs_setext");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
#[ignore = "known compat gap: html-block classification still pending"]
fn compat_incorrectly_formatted_list_and_hr_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("incorrectly_formatted_list_and_hr");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_nested_blockquote_in_list_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("nested_blockquote_in_list");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_list_code_header_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("list_code_header");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_tasklist_blocks_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("tasklist_blocks");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_pedantic_heading_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("pedantic_heading");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_pedantic_heading_interrupts_paragraph_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("pedantic_heading_interrupts_paragraph");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
#[ignore = "known compat gap: pedantic nested-list whitespace still differs from marked"]
fn compat_list_align_pedantic_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("list_align_pedantic");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_nogfm_hashtag_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("nogfm_hashtag");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_link_lt_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("link_lt");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_def_blocks_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("def_blocks");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_definition_multiline_title_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(196);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_definition_unicode_label_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(206);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_image_alt_text_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(573);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_autolink_scheme_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(598);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_definition_backslashes_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(202);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_definition_does_not_interrupt_paragraph_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(213);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blockquote_reference_definition_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(218);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_link_destination_entities_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(503);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_link_title_variants_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(505);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blockquote_list_continuation_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(259);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_tab_after_blockquote_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("tab_after_blockquote");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_inline_processing_instruction_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(627);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_inline_declaration_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(628);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_inline_cdata_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(629);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_html_block_closing_tag_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(151);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_html_block_processing_instruction_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(180);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_html_block_interrupts_paragraph_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(185);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_entity_references_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(25);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_numeric_entity_references_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(26);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_entities_do_not_trigger_emphasis_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(37);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_entities_do_not_trigger_list_markers_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(38);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_entities_preserve_literal_newlines_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(39);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_entities_decode_in_fenced_info_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(34);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_setext_multiline_heading_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(82);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_setext_single_equals_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(83);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_setext_trims_trailing_space_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(89);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_unquoted_thematic_break_does_not_stay_in_blockquote_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(92);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_unquoted_thematic_break_after_blockquote_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(101);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_setext_no_blankline_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("setext_no_blankline");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_lheading_following_table_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("lheading_following_table");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_inlinecode_following_tables_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("inlinecode_following_tables");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_text_following_tables_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("text_following_tables");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_nbsp_following_tables_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("nbsp_following_tables");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_whitespace_lines_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("whiltespace_lines");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_indented_code_blank_lines_match_marked() {
    let (markdown, expected) = commonmark_example_pair(111);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_fences_following_table_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("fences_following_table");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_fences_following_nptable_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("fences_following_nptable");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_table_cells_matches_marked() {
    let (markdown, expected) = compat_fixture_pair("table_cells");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_autolink_scheme_length_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(609);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_angle_autolink_backslash_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(20);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_angle_autolink_backtick_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(346);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_angle_autolink_escaped_brackets_match_marked() {
    let (markdown, expected) = commonmark_example_pair(603);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_escaped_bang_reference_link_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(593);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_bare_autolink_parentheses_match_marked() {
    let (markdown, expected) = gfm_example_pair(18);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_bare_autolink_entities_match_marked() {
    let (markdown, expected) = gfm_example_pair(19);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_original_links_inline_style_matches_marked() {
    let (markdown, expected) = compat_original_fixture_pair("links_inline_style");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_original_literal_quotes_in_titles_matches_marked() {
    let (markdown, expected) = compat_original_fixture_pair("literal_quotes_in_titles");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_original_markdown_documentation_basics_matches_marked() {
    let (markdown, expected) = compat_original_fixture_pair("markdown_documentation_basics");
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_bare_autolink_lt_boundary_matches_marked() {
    let (markdown, expected) = gfm_example_pair(20);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_scheme_email_autolinks_match_marked() {
    let (markdown, expected) = gfm_example_pair(25);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_scheme_email_path_autolinks_match_marked() {
    let (markdown, expected) = gfm_example_pair(27);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_disallowed_raw_html_matches_marked() {
    let (markdown, expected) = gfm_example_pair(28);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_label_casefold_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(540);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_link_does_not_cross_line_between_labels_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(543);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_first_reference_definition_wins_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(544);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_collapsed_reference_link_does_not_cross_line_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(556);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_invalid_reference_label_with_open_bracket_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(546);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_invalid_reference_label_with_nested_brackets_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(547);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_invalid_reference_definition_for_nested_shortcut_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(548);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_empty_reference_label_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(552);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_nested_inline_link_rejects_outer_link_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(518);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_nested_inline_image_alt_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(520);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_raw_html_does_not_close_link_label_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(524);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_autolink_does_not_close_link_label_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(526);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_outer_link_rejects_inner_link_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(532);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_space_between_reference_labels_is_not_allowed_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(542);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_escaped_table_pipes_matches_marked() {
    let (markdown, expected) = gfm_example_pair(3);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_table_header_delimiter_mismatch_matches_marked() {
    let (markdown, expected) = gfm_example_pair(6);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_header_only_table_matches_marked() {
    let (markdown, expected) = gfm_example_pair(8);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_gfm_triple_tilde_text_matches_marked() {
    let (markdown, expected) = gfm_example_pair(13);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_fenced_info_backslash_escape_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(24);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_invalid_long_numeric_entities_match_marked() {
    let (markdown, expected) = commonmark_example_pair(28);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_fenced_info_uses_first_word_for_language_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(143);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_fenced_info_ignores_trailing_tokens_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(146);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blockquote_fence_does_not_take_lazy_lines_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(237);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_indented_lazy_blockquote_line_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(238);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blank_blockquote_line_ends_lazy_continuation_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(249);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_nested_blockquote_lazy_lines_match_marked() {
    let (markdown, expected) = commonmark_example_pair(250);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_tabbed_list_code_continuation_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(5);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_tabbed_blockquote_code_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(6);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_tabbed_list_item_code_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(7);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_bare_list_marker_items_match_marked() {
    let (markdown, expected) = commonmark_example_pair(278);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blank_line_after_empty_list_item_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(280);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_non_one_ordered_list_does_not_interrupt_paragraph_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(304);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_paragraph_continuation_indent_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(223);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_final_paragraph_line_trailing_spaces_match_marked() {
    let (markdown, expected) = commonmark_example_pair(226);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_hard_break_continuation_indent_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(636);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_backslash_hard_break_continuation_indent_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(637);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_terminal_hard_break_spaces_do_not_render_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(645);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blockquote_indented_code_does_not_lazy_continue_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(236);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blockquote_list_item_lazy_continuation_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(292);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blank_line_before_nested_list_makes_item_loose_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(109);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_tight_list_heading_paragraph_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(300);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_nested_blank_lines_only_loosen_inner_list_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(307);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_reference_definition_loose_list_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(317);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_blank_lines_inside_fences_do_not_loosen_parent_list_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(318);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_underindented_continuation_marker_stays_literal_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(312);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}

#[test]
fn compat_commonmark_outer_list_stays_tight_when_inner_list_is_loose_matches_marked() {
    let (markdown, expected) = commonmark_example_pair(319);
    let actual = render_compat_fixture(&markdown);

    assert_eq!(normalize_html(&actual), normalize_html(&expected));
}
