use markrs::{RenderOptions, render_markdown_to_html};

#[test]
fn parser_blocks_parenthesized_ordered_lists_render() {
    let html = render_markdown_to_html("1) first\n2) second", RenderOptions::default());

    assert!(html.contains("<ol>"));
    assert!(html.contains("<li>first</li>"));
    assert!(html.contains("<li>second</li>"));
}

#[test]
fn parser_blocks_preserve_empty_table_body_cells() {
    let md = "| a | b | c |\n| --- | --- | --- |\n| 1 |   | 3 |";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<table>"));
    assert!(html.contains("<td></td>"));
}

#[test]
fn parser_blocks_support_single_column_tables_without_pipes() {
    let md = "table\n:----\nvalue\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<table>"));
    assert!(html.contains("<th align=\"left\">table</th>"));
    assert!(html.contains("<td align=\"left\">value</td>"));
}

#[test]
fn parser_blocks_preserve_escaped_pipes_and_code_spans_in_table_cells() {
    let md = "| f\\|oo  |\n| ------ |\n| b `\\|` az |\n| b **\\|** im |\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<th>f|oo</th>"));
    assert!(html.contains("<td>b <code>|</code> az</td>"));
    assert!(html.contains("<td>b <strong>|</strong> im</td>"));
}

#[test]
fn parser_blocks_reject_table_when_header_and_delimiter_column_counts_differ() {
    let md = "| abc | def |\n| --- |\n| bar |\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(!html.contains("<table>"));
    assert!(html.contains("<p>| abc | def |\n| --- |\n| bar |</p>"));
}

#[test]
fn parser_blocks_render_header_only_tables_without_empty_tbody() {
    let md = "| abc | def |\n| --- | --- |\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<table><thead><tr><th>abc</th><th>def</th></tr></thead></table>"));
    assert!(!html.contains("<tbody>"));
}

#[test]
fn parser_blocks_prefer_setext_over_pipe_header_and_plain_dash_line() {
    let md = "| setext |\n----------\n| setext |\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<h2>| setext |</h2>"));
    assert!(!html.contains("<table>"));
}

#[test]
fn parser_blocks_disable_tables_in_non_gfm_mode() {
    let md = "| a | b |\n| - | - |\n| 1 | 2 |\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            breaks: false,
            pedantic: false,
        },
    );

    assert!(!html.contains("<table>"));
}

#[test]
fn parser_blocks_keep_indented_thematic_breaks_out_of_hr() {
    let html = render_markdown_to_html("    ---", RenderOptions::default());

    assert!(!html.contains("<hr>"));
    assert!(html.contains("<pre>") || html.contains("<p>"));
}

#[test]
fn parser_blocks_render_loose_lists_with_paragraph_wrappers() {
    let md = "- item 1\n-\n  item 2\n\n  still item 2\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<li><p>item 1</p>\n</li>"));
    assert!(html.contains("<li><p>item 2</p>"));
    assert!(html.contains("<p>still item 2</p>"));
}

#[test]
fn parser_blocks_end_list_items_before_following_top_level_paragraphs() {
    let md = "- ***\nparagraph\n- # heading\nparagraph\n-     indented code\nparagraph\n- ```\n  fenced code\n  ```\nparagraph\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html.matches("<ul>").count(), 4);
    assert_eq!(html.matches("<p>paragraph</p>").count(), 4);
    assert!(html.contains("<li><hr>\n</li></ul>\n<p>paragraph</p>"));
    assert!(html.contains("<li><h1>heading</h1>\n</li></ul>\n<p>paragraph</p>"));
    assert!(html.contains("<li><pre><code>indented code"));
    assert!(html.contains("<li><pre><code>fenced code"));
}

#[test]
fn parser_blocks_keep_task_markers_literal_when_gfm_is_disabled() {
    let html = render_markdown_to_html(
        "- [ ] A\n- [x] B\n- [ ] C\n",
        RenderOptions {
            gfm: false,
            breaks: false,
            pedantic: false,
        },
    );

    assert!(!html.contains("checkbox"));
    assert!(html.contains("[ ] A"));
    assert!(html.contains("[x] B"));
}

#[test]
fn parser_blocks_keep_task_item_first_line_block_markers_literal() {
    let md = "- [x] # heading\n- [x] > blockquote\n- [x] [def]: https://example.com\n- [x] | a | b |\n  |---|---|\n  | 1 | 1 |\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(!html.contains("<li><h1>heading</h1>"));
    assert!(!html.contains("<blockquote>"));
    assert!(!html.contains("<table>"));
    assert!(
        html.contains("<li><input type=\"checkbox\" checked=\"\" disabled=\"\"> # heading</li>")
    );
    assert!(
        html.contains(
            "<li><input type=\"checkbox\" checked=\"\" disabled=\"\"> &gt; blockquote</li>"
        )
    );
    assert!(html.contains(
        "<li><input type=\"checkbox\" checked=\"\" disabled=\"\"> [def]: <a href=\"https://example.com\">https://example.com</a></li>"
    ));
    assert!(html.contains(
        "<li><input type=\"checkbox\" checked=\"\" disabled=\"\"> | a | b |\n|---|---|\n| 1 | 1 |</li>"
    ));
}

#[test]
fn parser_blocks_end_nested_list_items_before_parent_blockquotes() {
    let md = "- list item\n  - nested list item\n  > quoteblock\n\n- list item\n  - nested list item\n> quote block\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<ul><li>nested list item</li></ul>\n<blockquote>"));
    assert!(html.contains("</ul>\n</li></ul>\n<blockquote><p>quote block</p>"));
}

#[test]
fn parser_blocks_split_unordered_lists_when_marker_changes() {
    let md = "* alpha\n- beta\n+ gamma\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html.matches("<ul>").count(), 3);
    assert!(
        html.contains("<ul><li>alpha</li></ul>\n<ul><li>beta</li></ul>\n<ul><li>gamma</li></ul>")
    );
}

#[test]
fn parser_blocks_keep_inline_html_inside_paragraphs_for_setext() {
    let md = "<b>heading</b>\n-----\n\n<s>not heading</s>\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<h2><b>heading</b></h2>"));
    assert!(html.contains("<p><s>not heading</s></p>"));
}

#[test]
fn parser_blocks_merge_mixed_bullets_in_pedantic_mode() {
    let html = render_markdown_to_html(
        "* alpha\n+ beta\n- gamma\n",
        RenderOptions {
            gfm: true,
            breaks: false,
            pedantic: true,
        },
    );

    assert_eq!(html.matches("<ul>").count(), 1);
    assert!(html.contains("<li>alpha</li>"));
    assert!(html.contains("<li>beta</li>"));
    assert!(html.contains("<li>gamma</li>"));
}

#[test]
fn parser_blocks_keep_multiline_html_open_tags_as_blocks() {
    let md = "<div id=\"foo\"\n  class=\"bar\">\n</div>\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(!html.contains("<p><div"));
    assert!(html.contains("<div id=\"foo\"\n  class=\"bar\">\n</div>"));
}

#[test]
fn parser_blocks_do_not_treat_indented_code_as_setext_heading_text() {
    let md = "# Heading\n    foo\nHeading\n------\n    foo\n----\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<h1>Heading</h1>"));
    assert!(html.contains("<h2>Heading</h2>"));
    assert_eq!(html.matches("<pre><code>foo").count(), 2);
    assert!(html.contains("<hr>"));
}

#[test]
fn parser_blocks_keep_blockquote_indentation_for_list_continuations() {
    let md = "   > > 1.  one\n>>\n>>     two\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<blockquote><blockquote><ol><li><p>one</p>\n<p>two</p>\n</li></ol>\n</blockquote>\n</blockquote>"));
}

#[test]
fn parser_blocks_strip_fence_indent_from_list_code_contents() {
    let md = "1. item\n\n\t```\n\tconst x = 5;\n\tconst y = x + 5;\n\t```\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<pre><code>const x = 5;\nconst y = x + 5;\n</code></pre>"));
    assert!(!html.contains("<pre><code> const x = 5;"));
}

#[test]
fn parser_blocks_treat_tab_after_blockquote_marker_as_marker_padding() {
    let html = render_markdown_to_html(">\ttest\n", RenderOptions::default());

    assert_eq!(html, "<blockquote><p>test</p>\n</blockquote>\n");
}

#[test]
fn parser_blocks_preserve_tab_overhang_after_blockquote_marker_for_code_blocks() {
    let html = render_markdown_to_html(">\t\tfoo\n", RenderOptions::default());

    assert_eq!(
        html,
        "<blockquote><pre><code>  foo\n</code></pre>\n</blockquote>\n"
    );
}

#[test]
fn parser_blocks_preserve_tab_overhang_after_list_marker_for_code_blocks() {
    let html = render_markdown_to_html("-\t\tfoo\n", RenderOptions::default());

    assert_eq!(
        html,
        "<ul><li><pre><code>  foo\n</code></pre>\n</li></ul>\n"
    );
}

#[test]
fn parser_blocks_preserve_tab_overhang_on_list_code_continuations() {
    let html = render_markdown_to_html("- foo\n\n\t\tbar\n", RenderOptions::default());

    assert_eq!(
        html,
        "<ul><li><p>foo</p>\n<pre><code>  bar\n</code></pre>\n</li></ul>\n"
    );
}

#[test]
fn parser_blocks_do_not_let_non_one_ordered_lists_interrupt_paragraphs() {
    let md = "The number of windows in my house is\n14.  The number of doors is 6.\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(
        html,
        "<p>The number of windows in my house is\n14.  The number of doors is 6.</p>\n"
    );
}

#[test]
fn parser_blocks_empty_list_item_after_blank_line_does_not_absorb_paragraph() {
    let md = "-\n\n  foo\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html, "<ul><li></li></ul>\n<p>foo</p>\n");
}

#[test]
fn parser_blocks_bare_list_marker_does_not_start_setext_heading() {
    let md = "-\n  foo\n-\n  ```\n  bar\n  ```\n-\n      baz\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(
        html,
        "<ul><li>foo</li><li><pre><code>bar\n</code></pre>\n</li><li><pre><code>baz\n</code></pre>\n</li></ul>\n"
    );
}

#[test]
fn parser_blocks_strip_all_continuation_indent_from_paragraph_lines() {
    let md = "aaa\n             bbb\n                                       ccc\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html, "<p>aaa\nbbb\nccc</p>\n");
}

#[test]
fn parser_blocks_trim_trailing_spaces_from_final_paragraph_line() {
    let md = "foo  \n     bar\n\nfoo  \n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html, "<p>foo<br>\nbar</p>\n<p>foo</p>\n");
}

#[test]
fn parser_blocks_do_not_treat_unquoted_lines_as_lazy_inside_blockquote_fences() {
    let md = "> ```\nfoo\n```\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.starts_with("<blockquote><pre><code>"));
    assert!(
        html.contains("</code></pre>\n</blockquote>\n<p>foo</p>\n<pre><code>\n</code></pre>\n")
    );
}

#[test]
fn parser_blocks_do_not_lazy_continue_after_blockquote_indented_code() {
    let md = ">     foo\n    bar\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.starts_with("<blockquote><pre><code>foo\n</code></pre>\n</blockquote>\n"));
    assert!(html.contains("</blockquote>\n<pre><code>bar\n"));
}

#[test]
fn parser_blocks_allow_indented_lazy_lines_in_blockquote_paragraphs() {
    let md = "> foo\n    - bar\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html, "<blockquote><p>foo\n- bar</p>\n</blockquote>\n");
}

#[test]
fn parser_blocks_blank_blockquote_line_stops_lazy_continuation() {
    let md = "> bar\n>\nbaz\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html, "<blockquote><p>bar</p>\n</blockquote>\n<p>baz</p>\n");
}

#[test]
fn parser_blocks_nested_blockquote_lazy_lines_do_not_leak_prefix_markers() {
    let md = "> > > foo\nbar\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(
        html,
        "<blockquote><blockquote><blockquote><p>foo\nbar</p>\n</blockquote>\n</blockquote>\n</blockquote>\n"
    );
}

#[test]
fn parser_blocks_keep_html_blocks_open_until_blank_line() {
    let md = "</div>\n*foo*\n\n<div></div>\n``` c\nint x = 33;\n```\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("</div>\n*foo*"));
    assert!(html.contains("<div></div>\n``` c\nint x = 33;\n```"));
}

#[test]
fn parser_blocks_support_processing_instruction_and_cdata_html_blocks() {
    let md = "<?php\n\necho '>';\n\n?>\nokay\n\n<!DOCTYPE html>\n\n<![CDATA[\ncontent\n]]>\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<?php\n\necho '>';\n\n?>"));
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<![CDATA[\ncontent\n]]>"));
    assert!(html.contains("<p>okay</p>"));
}

#[test]
fn parser_blocks_decode_entities_in_fenced_code_info_string() {
    let html = render_markdown_to_html("``` f&ouml;&ouml;\nbody\n```\n", RenderOptions::default());

    assert!(html.contains("<code class=\"language-föö\">body\n</code>"));
}

#[test]
fn parser_blocks_use_only_first_fenced_info_word_for_language_class() {
    let html = render_markdown_to_html(
        "``` foo\\+bar extra words\nbody\n```\n",
        RenderOptions::default(),
    );

    assert!(html.contains("<code class=\"language-foo+bar\">body\n</code>"));
    assert!(!html.contains("language-foo\\+bar extra words"));
}

#[test]
fn parser_blocks_leave_too_long_numeric_entities_literal() {
    let html = render_markdown_to_html("&#87654321;\n&#xabcdef0;\n", RenderOptions::default());

    assert_eq!(html, "<p>&amp;#87654321;\n&amp;#xabcdef0;</p>\n");
}

#[test]
fn parser_blocks_trim_setext_heading_content_indent_and_trailing_space() {
    let md = "  Foo *bar\nbaz*\t\n====\n\nFoo  \n-----\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<h1>Foo <em>bar\nbaz</em></h1>"));
    assert!(html.contains("<h2>Foo</h2>"));
}

#[test]
fn parser_blocks_allow_single_equals_setext_and_break_unquoted_hr_after_blockquote() {
    let md = "Foo\n=\n\n> Foo\n---\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<h1>Foo</h1>"));
    assert!(html.contains("<blockquote><p>Foo</p>\n</blockquote>\n<hr>"));
}

#[test]
fn parser_blocks_do_not_parse_block_html_as_setext_heading_text() {
    let md = "<html>\n=\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html, "<html>\n=\n");
}

#[test]
fn parser_blocks_keep_whitespace_only_blank_lines_inside_indented_code_blocks() {
    let md = "    a\n      \n    b\n  \n    c\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html.matches("<pre><code>").count(), 1);
    assert!(html.contains("<pre><code>a\n\nb\n\nc\n"));
}

#[test]
fn parser_blocks_extend_tables_with_implicit_tail_rows() {
    let md = "| abc | def |\n| --- | --- |\n| bar | foo |\nhello\n**strong**\n`code`\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<tr><td>hello</td><td></td></tr>"));
    assert!(html.contains("<tr><td><strong>strong</strong></td><td></td></tr>"));
    assert!(html.contains("<tr><td><code>code</code></td><td></td></tr>"));
    assert!(!html.contains("</table><p>hello</p>"));
}

#[test]
fn parser_blocks_stop_table_before_following_blocks() {
    let md = "| abc | def |\n| --- | --- |\n| bar | foo |\n# heading\n> quote\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("</table>\n<h1>heading</h1>"));
    assert!(html.contains("<blockquote><p>quote</p>\n</blockquote>"));
}

#[test]
fn parser_blocks_stop_table_before_following_fences_without_extra_code_newline() {
    let with_pipes =
        "| abc | def |\n| --- | --- |\n| bar | foo |\n| baz | boo |\n```\nfoobar()\n```\n";
    let no_pipes = " abc | def\n --- | ---\n bar | foo\n baz | boo\n```\nfoobar()\n```\n";

    let html_with_pipes = render_markdown_to_html(with_pipes, RenderOptions::default());
    let html_no_pipes = render_markdown_to_html(no_pipes, RenderOptions::default());

    assert!(html_with_pipes.contains("</table>\n<pre><code>foobar()</code></pre>"));
    assert!(html_no_pipes.contains("</table>\n<pre><code>foobar()</code></pre>"));
}

#[test]
fn parser_blocks_treat_nbsp_tail_after_table_as_empty_row() {
    let md = "| abc | def |\n| --- | --- |\n| bar | foo |\n\u{00A0}\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<tr><td></td><td></td></tr>"));
}

#[test]
fn parser_blocks_accept_space_then_tab_after_list_marker() {
    let md = "1. \tSomeText\n2. \tSomeText\n\n- \tSomeText\n- \tSomeText\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<ol><li>SomeText</li><li>SomeText</li></ol>"));
    assert!(html.contains("<ul><li>SomeText</li><li>SomeText</li></ul>"));
    assert!(!html.contains("<pre><code>SomeText"));
}

#[test]
fn parser_blocks_keep_tab_indented_list_paragraphs_inside_items() {
    let md = "1.\tFirst\n\n\tSecond paragraph\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<ol><li><p>First</p>\n<p>Second paragraph</p>\n</li></ol>"));
    assert!(!html.contains("<pre><code>Second paragraph"));
}

#[test]
fn parser_blocks_keep_tab_indented_nested_lists_nested() {
    let md = "*\tTab\n\t*\tTab\n\t\t*\tTab\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(html.matches("<ul>").count(), 3);
    assert_eq!(html.matches("<li>Tab").count(), 3);
    assert!(!html.contains("<pre><code>"));
}

#[test]
fn parser_blocks_keep_pedantic_text_after_nested_list_inside_item() {
    let md = "  * item1\n\n    * item2\n\n  text\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<ul><li><p>item1</p>"));
    assert!(html.contains("<ul><li>item2</li></ul>"));
    assert!(html.contains("<p>text</p>\n</li></ul>"));
    assert!(!html.ends_with("<p>text</p>\n"));
}

#[test]
fn parser_blocks_pedantic_list_alignment_matches_marked_shape() {
    let md = "- one\n - two\n  - three\n    - four\n     - five\n      - six\n       - seven\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<ul><li>one\n<ul><li>two</li><li>three</li><li>four"));
    assert!(html.contains("<ul><li>five</li><li>six</li><li>seven</li></ul>"));
    assert!(!html.starts_with("<ul><li>one</li><li>two</li>"));
}

#[test]
fn parser_blocks_do_not_swallow_following_reference_definitions_into_titles() {
    let md = "[s]: /syntax  \"Markdown Syntax\"\n  [d]: /dingus  \"Markdown Dingus\"\n\n[syntax page] [s]\n\n[Dingus] [d]\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<a href=\"/syntax\" title=\"Markdown Syntax\">syntax page</a>"));
    assert!(html.contains("<a href=\"/dingus\" title=\"Markdown Dingus\">Dingus</a>"));
    assert!(!html.contains("[d]: /dingus"));
}

#[test]
fn parser_blocks_parse_pedantic_hash_headings_without_space() {
    let md = "#h1\n\n#h1#\n\n#h1 # #\n\n#h1####\n\n # h1\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<h1>h1</h1>"));
    assert!(html.contains("<h1>h1 #</h1>"));
    assert!(html.contains("<p># h1</p>"));
}

#[test]
fn parser_blocks_pedantic_hash_heading_interrupts_paragraph() {
    let md = "paragraph before head with hash\n#how are you\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<p>paragraph before head with hash</p>\n<h1>how are you</h1>"));
}

#[test]
fn parser_blocks_pedantic_setext_heading_does_not_absorb_previous_paragraph_line() {
    let md = "paragraph before head with equals\nhow are you again\n===========\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<p>paragraph before head with equals</p>\n<h1>how are you again</h1>"));
}

#[test]
fn parser_blocks_parse_pedantic_headings_without_gfm() {
    let md = "#header\n\n# header1\n\n#  header2\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert_eq!(html.matches("<h1>").count(), 3);
    assert!(html.contains("<h1>header</h1>"));
    assert!(html.contains("<h1>header1</h1>"));
    assert!(html.contains("<h1>header2</h1>"));
}

#[test]
fn parser_blocks_allow_blockquote_lazy_continuation_inside_list_items() {
    let md = "> 1. > Blockquote\ncontinued here.\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<blockquote><ol><li><blockquote><p>Blockquote\ncontinued here.</p>"));
    assert!(!html.ends_with("<p>continued here.</p>\n"));
}

#[test]
fn parser_blocks_flatten_tight_list_paragraphs_after_headings() {
    let md = "- # Foo\n- Bar\n  ---\n  baz\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<li><h2>Bar</h2>\nbaz</li>"));
    assert!(!html.contains("<h2>Bar</h2>\n<p>baz</p>"));
}

#[test]
fn parser_blocks_keep_outer_lists_tight_when_blank_lines_are_nested_deeper() {
    let md = "- foo\n  - bar\n    - baz\n\n\n      bim\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<ul><li>foo\n<ul><li>bar\n<ul><li><p>baz</p>\n<p>bim</p>\n</li></ul>\n</li></ul>\n</li></ul>"));
    assert!(!html.contains("<li><p>foo</p>"));
    assert!(!html.contains("<li><p>bar</p>"));
}

#[test]
fn parser_blocks_reference_definitions_after_blank_lines_loosen_lists() {
    let md = "- a\n- b\n\n  [ref]: /url\n- d\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<li><p>a</p>\n</li><li><p>b</p>\n</li><li><p>d</p>\n</li>"));
}

#[test]
fn parser_blocks_blank_line_before_nested_list_makes_item_loose() {
    let md = "1.  foo\n\n    - bar\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<ol><li><p>foo</p>\n<ul><li>bar</li></ul>\n</li></ol>"));
}

#[test]
fn parser_blocks_blank_lines_inside_fenced_code_do_not_loosen_parent_list() {
    let md = "- a\n- ```\n  b\n\n\n  ```\n- c\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(
        html.contains("<ul><li>a</li><li><pre><code>b\n\n\n</code></pre>\n</li><li>c</li></ul>")
    );
    assert!(!html.contains("<li><p>a</p>"));
    assert!(!html.contains("<li><p>c</p>"));
}

#[test]
fn parser_blocks_underindented_continuation_markers_stay_literal_text() {
    let md = "- a\n - b\n  - c\n   - d\n    - e\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<ul><li>a</li><li>b</li><li>c</li><li>d\n- e</li></ul>"));
    assert_eq!(html.matches("<ul>").count(), 1);
}

#[test]
fn parser_blocks_keep_parent_lists_tight_when_only_nested_lists_are_loose() {
    let md = "- a\n  - b\n\n    c\n- d\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(
        html.contains("<ul><li>a\n<ul><li><p>b</p>\n<p>c</p>\n</li></ul>\n</li><li>d</li></ul>")
    );
    assert!(!html.contains("<li><p>a</p>"));
    assert!(!html.contains("<li><p>d</p>"));
}
