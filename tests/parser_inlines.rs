use markrs::{RenderOptions, render_markdown_to_html};

#[test]
fn parser_inlines_render_link_and_image_titles() {
    let md = "[github](https://github.com \"GitHub\")\n\n![logo](logo.png 'Markec logo')";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"https://github.com\" title=\"GitHub\">github</a>"));
    assert!(html.contains("<img "));
    assert!(html.contains("src=\"logo.png\""));
    assert!(html.contains("alt=\"logo\""));
    assert!(html.contains("title=\"Markec logo\""));
}

#[test]
fn parser_inlines_pedantic_links_allow_spacey_destinations_and_literal_quotes() {
    let md = "[space]( /url/has space )\n\n[space title]( /url/has space/ \"title here\")\n\n[quoted](/url/ \"Title with \"quotes\" inside\")";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(html.contains("<a href=\"/url/has%20space\">space</a>"));
    assert!(html.contains("<a href=\"/url/has%20space/\" title=\"title here\">space title</a>"));
    assert!(
        html.contains(
            "<a href=\"/url/\" title=\"Title with &quot;quotes&quot; inside\">quoted</a>"
        )
    );
}

#[test]
fn parser_inlines_escaped_bang_still_allows_reference_links() {
    let md = "\\![foo]\n\n[foo]: /url \"title\"";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert_eq!(
        html.trim(),
        "<p>!<a href=\"/url\" title=\"title\">foo</a></p>"
    );
}

#[test]
fn parser_inlines_render_parenthesized_and_entity_titles() {
    let md = "[link](/url (title))\n\n[link](/url \"title \\\"&quot;\")";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"/url\" title=\"title\">link</a>"));
    assert!(html.contains("<a href=\"/url\" title=\"title &quot;&quot;\">link</a>"));
}

#[test]
fn parser_inlines_do_not_cross_line_between_reference_labels() {
    let md = "[alpha]\n[bar]\n\n[bar]: /url \"title\"\n\n[foo] \n[]\n\n[foo]: /url \"title\"";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p>[alpha]"));
    assert!(html.contains("<a href=\"/url\" title=\"title\">bar</a></p>"));
    assert!(html.contains("<a href=\"/url\" title=\"title\">foo</a>"));
    assert!(html.contains("[]</p>"));
}

#[test]
fn parser_inlines_allow_reference_label_whitespace_only_in_pedantic_mode() {
    let md = "Foo [bar] [1].\n\nAnd [this] [].\n\n[1]: /url/ \"Title\"\n[this]: foo";
    let default_html = render_markdown_to_html(md, RenderOptions::default());
    let pedantic_html = render_markdown_to_html(
        md,
        RenderOptions {
            pedantic: true,
            ..RenderOptions::default()
        },
    );

    assert!(default_html.contains("<p>Foo [bar] <a href=\"/url/\" title=\"Title\">1</a>.</p>"));
    assert!(default_html.contains("<p>And <a href=\"foo\">this</a> [].</p>"));
    assert!(pedantic_html.contains("<a href=\"/url/\" title=\"Title\">bar</a>"));
    assert!(pedantic_html.contains("<a href=\"foo\">this</a>."));
}

#[test]
fn parser_inlines_render_reference_links_with_escaped_newlines() {
    let md = "[foo\\\nbar]\n\n[foo\\\nbar]: https://example.com\n\n[foo3\\\nbar3][foo\\\nbar]";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"https://example.com\">foo<br>\nbar</a>"));
    assert!(html.contains("<a href=\"https://example.com\">foo3<br>\nbar3</a>"));
}

#[test]
fn parser_inlines_render_angle_autolink_email() {
    let html = render_markdown_to_html("<hello@example.com>", RenderOptions::default());

    assert!(html.contains("<a href=\"mailto:hello@example.com\">hello@example.com</a>"));
}

#[test]
fn parser_inlines_render_generic_scheme_autolinks() {
    let html = render_markdown_to_html("<a+b+c:d>", RenderOptions::default());

    assert!(html.contains("<a href=\"a+b+c:d\">a+b+c:d</a>"));
}

#[test]
fn parser_inlines_preserve_literal_backslashes_and_backticks_in_angle_autolinks() {
    let html = render_markdown_to_html(
        "<https://example.com/\\[\\>\n\n<https://foo.bar.`baz>\n\n<https://example.com?find=\\*>",
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(
        html.contains("<a href=\"https://example.com/%5C%5B%5C\">https://example.com/\\[\\</a>")
    );
    assert!(html.contains("<a href=\"https://foo.bar.%60baz\">https://foo.bar.`baz</a>"));
    assert!(
        html.contains("<a href=\"https://example.com?find=%5C*\">https://example.com?find=\\*</a>")
    );
}

#[test]
fn parser_inlines_do_not_render_spaced_angle_brackets_as_autolinks() {
    let html = render_markdown_to_html(
        "< https://foo.bar >",
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert_eq!(html.trim(), "<p>&lt; https://foo.bar &gt;</p>");
}

#[test]
fn parser_inlines_match_marked_gfm_bare_autolink_edges() {
    let md = "www.google.com/search?q=(business))+ok\n\nwww.google.com/search?q=commonmark&hl=en\n\nwww.google.com/search?q=commonmark&hl;\n\nxmpp:foo@bar.baz/txt/bin";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains(
        "<a href=\"http://www.google.com/search?q=(business))+ok\">www.google.com/search?q=(business))+ok</a>"
    ));
    assert!(html.contains(
        "<a href=\"http://www.google.com/search?q=commonmark&amp;hl=en\">www.google.com/search?q=commonmark&amp;hl=en</a>"
    ));
    assert!(html.contains(
        "<a href=\"http://www.google.com/search?q=commonmark\">www.google.com/search?q=commonmark</a>&amp;hl;"
    ));
    assert!(html.contains("<a href=\"xmpp:foo@bar.baz/txt\">xmpp:foo@bar.baz/txt</a>/bin"));
}

#[test]
fn parser_inlines_do_not_treat_triple_tilde_runs_as_nested_strikethrough() {
    let html = render_markdown_to_html("This will ~~~not~~~ strike.", RenderOptions::default());

    assert_eq!(html.trim(), "<p>This will ~~~not~~~ strike.</p>");
}

#[test]
fn parser_inlines_escape_gfm_disallowed_raw_html_tags() {
    let md = "<strong> <title> <style> <em>\n\n<blockquote>\n  <xmp> is disallowed.\n</blockquote>";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p><strong> &lt;title> &lt;style> <em></p>"));
    assert!(html.contains("<blockquote>\n  &lt;xmp> is disallowed.\n</blockquote>"));
}

#[test]
fn parser_inlines_render_unicode_reference_labels() {
    let md = "[ΑΓΩ]: /φου\n\n[αγω]";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"/%CF%86%CE%BF%CF%85\">αγω</a>"));
}

#[test]
fn parser_inlines_render_multiline_reference_titles() {
    let md = "[foo]: /url '\ntitle\nline1\nline2\n'\n\n[foo]";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"/url\" title=\"\ntitle\nline1\nline2\n\">foo</a>"));
}

#[test]
fn parser_inlines_render_reference_images_with_flattened_alt_text() {
    let md = "![foo *bar*][]\n\n[foo *bar*]: train.jpg \"train & tracks\"";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<img src=\"train.jpg\" alt=\"foo bar\" title=\"train &amp; tracks\">"));
}

#[test]
fn parser_inlines_render_escaped_and_entity_destinations() {
    let md = "[link](foo\\)\\:)\n\n[link](foo\\bar)\n\n[link](foo%20b&auml;)";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"foo):\">link</a>"));
    assert!(html.contains("<a href=\"foo%5Cbar\">link</a>"));
    assert!(html.contains("<a href=\"foo%20b%C3%A4\">link</a>"));
}

#[test]
fn parser_inlines_keep_nbsp_inside_link_destination() {
    let md = "[link](/url\u{00A0}\"title\")";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"/url%C2%A0%22title%22\">link</a>"));
}

#[test]
fn parser_inlines_support_nested_emphasis_and_strong() {
    let md = "*test **test** test*";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(
        html.trim(),
        "<p><em>test <strong>test</strong> test</em></p>"
    );
}

#[test]
fn parser_inlines_support_complex_emphasis_and_strong_nesting() {
    let md = "**E*mp****ha****si*s**";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert_eq!(
        html.trim(),
        "<p><strong>E<em>mp</em><em><strong>ha</strong></em><em>si</em>s</strong></p>"
    );
}

#[test]
fn parser_inlines_support_emoji_wrapped_by_quad_and_triple_stars() {
    let md = "***💁 test***\n\n****💁 test****";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p><em><strong>💁 test</strong></em></p>"));
    assert!(html.contains("<p><strong><strong>💁 test</strong></strong></p>"));
}

#[test]
fn parser_inlines_support_strikethrough_inside_emphasis_and_strong() {
    let md = "*~a~*b\n\n**~~a~~**b";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p><em><del>a</del></em>b</p>"));
    assert!(html.contains("<p><strong><del>a</del></strong>b</p>"));
}

#[test]
fn parser_inlines_support_emphasis_around_strikethrough_after_prefix_text() {
    let md = "b*~a~*b\n\na~a~*@*";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p>b<em><del>a</del></em>b</p>"));
    assert!(html.contains("<p>a<del>a</del><em>@</em></p>"));
}

#[test]
fn parser_inlines_do_not_mismatch_single_and_double_tildes() {
    let md = "~~test~\n\n~test~~";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p>~~test~</p>"));
    assert!(html.contains("<p>~test~~</p>"));
}

#[test]
fn parser_inlines_reject_outer_links_when_label_contains_link() {
    let md = "[foo [bar](/uri)](/uri)\n\n[foo *bar [baz][ref]*][ref]\n\n[ref]: /uri";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p>[foo <a href=\"/uri\">bar</a>](/uri)</p>"));
    assert!(
        html.contains("<p>[foo <em>bar <a href=\"/uri\">baz</a></em>]<a href=\"/uri\">ref</a></p>")
    );
}

#[test]
fn parser_inlines_skip_raw_html_and_autolinks_when_matching_link_brackets() {
    let md = "[foo<https://example.com/?search=](uri)>";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: false,
            ..RenderOptions::default()
        },
    );

    assert!(
        html.contains("<p>[foo<a href=\"https://example.com/?search=%5D(uri)\">https://example.com/?search=](uri)</a></p>")
    );
}

#[test]
fn parser_inlines_preserve_outer_image_text_when_alt_contains_nested_link() {
    let html = render_markdown_to_html("![[[foo](uri1)](uri2)](uri3)", RenderOptions::default());

    assert!(html.contains("<img src=\"uri3\" alt=\"[foo](uri2)\">"));
}

#[test]
fn parser_inlines_render_processing_instruction_and_declaration_html() {
    let html = render_markdown_to_html(
        "foo <?php echo $a; ?>\nfoo <!ELEMENT br EMPTY>\nfoo <![CDATA[>&<]]>\n",
        RenderOptions::default(),
    );

    assert!(html.contains("<?php echo $a; ?>"));
    assert!(html.contains("<!ELEMENT br EMPTY>"));
    assert!(html.contains("<![CDATA[>&<]]>"));
}

#[test]
fn parser_inlines_escape_invalid_html_tags() {
    let html = render_markdown_to_html(
        "<a h*#ref=\"hi\">\n<a href='bar'title=title>\n</a href=\"foo\">\n",
        RenderOptions::default(),
    );

    assert!(html.contains("&lt;a h*#ref=&quot;hi&quot;&gt;"));
    assert!(html.contains("&lt;a href='bar'title=title&gt;"));
    assert!(html.contains("&lt;/a href=&quot;foo&quot;&gt;"));
}

#[test]
fn parser_inlines_decode_named_and_numeric_entities_as_literal_text() {
    let md = "&copy; &AElig; &Dcaron; &frac34; &HilbertSpace; &DifferentialD; &ClockwiseContourIntegral; &ngE;\n\n&#35; &#1234; &#992; &#0;\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<p>© Æ Ď ¾ ℋ ⅆ ∲ ≧̸</p>"));
    assert!(html.contains("<p># Ӓ Ϡ �</p>"));
}

#[test]
fn parser_inlines_do_not_treat_entity_decoded_markers_as_emphasis() {
    let html = render_markdown_to_html("&#42;foo&#42;\n\n&#42; foo\n", RenderOptions::default());

    assert!(html.contains("<p>*foo*</p>"));
    assert!(html.contains("<p>* foo</p>"));
}

#[test]
fn parser_inlines_preserve_entity_decoded_newlines_and_tabs_as_text() {
    let html = render_markdown_to_html("foo&#10;&#10;bar\n\n&#9;foo\n", RenderOptions::default());

    assert!(html.contains("<p>foo\n\nbar</p>"));
    assert!(html.contains("<p>\tfoo</p>"));
}
