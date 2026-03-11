use markrs::{RenderOptions, ThemeFile, build_html_document, render_markdown_to_html};

#[test]
fn own_fragment_renders_gfm_features() {
    let md = "# Title\n\n- [x] done\n\n~~old~~\n\n| a | b |\n| - | - |\n| 1 | 2 |\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<h1>Title</h1>"));
    assert!(html.contains("type=\"checkbox\""));
    assert!(html.contains("<del>old</del>"));
    assert!(html.contains("<table>"));
}

#[test]
fn own_document_applies_theme_override_and_css() {
    let mut theme = ThemeFile::default();
    theme
        .variables
        .insert("--markrs-link".to_string(), "#ff3300".to_string());
    theme.css = Some(".markrs p { letter-spacing: 0.02em; }".to_string());

    let doc = build_html_document(
        "<p>Hello</p>",
        "github",
        Some(theme),
        Some(".x { color: red; }"),
    );

    assert!(doc.contains("--markrs-link: #ff3300;"));
    assert!(doc.contains(".markrs p { letter-spacing: 0.02em; }"));
    assert!(doc.contains(".x { color: red; }"));
    assert!(doc.contains("<main class=\"markrs\">"));
}

#[test]
fn own_non_gfm_mode_disables_tables() {
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
fn own_gfm_autolinks_plain_urls_and_emails() {
    let md = "www.example.com\n\n~~hello@email.com~~\n\nWww.example.com\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<a href=\"http://www.example.com\">www.example.com</a>"));
    assert!(html.contains("<del><a href=\"mailto:hello@email.com\">hello@email.com</a></del>"));
    assert!(html.contains("<p>Www.example.com</p>"));
}

#[test]
fn own_breaks_option_converts_softbreaks_to_br() {
    let md = "A\nB\n";
    let html = render_markdown_to_html(
        md,
        RenderOptions {
            gfm: true,
            breaks: true,
            pedantic: false,
        },
    );
    assert!(html.contains("<p>A<br>\nB</p>"));
}

#[test]
fn own_link_and_image_title_attributes() {
    let md = "[github](https://github.com \"GitHub\")\n\n![logo](logo.png 'Markec logo')";
    let html = render_markdown_to_html(md, RenderOptions::default());
    assert!(html.contains("<a href=\"https://github.com\" title=\"GitHub\">github</a>"));
    assert!(html.contains("<img "));
    assert!(html.contains("src=\"logo.png\""));
    assert!(html.contains("alt=\"logo\""));
    assert!(html.contains("title=\"Markec logo\""));
}

#[test]
fn own_parenthesized_ordered_list_is_parsed() {
    let md = "1) first\n2) second";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<ol>"));
    assert!(html.contains("<li>first</li>"));
    assert!(html.contains("<li>second</li>"));
}

#[test]
fn own_tight_list_item_joins_heading_without_separator() {
    let md = "- list\n  # header\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<li>list<h1>header</h1>\n</li>"));
}

#[test]
fn own_indented_thematic_break_is_not_hr() {
    let md = "    ---";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(!html.contains("<hr>"));
    assert!(html.contains("<pre>") || html.contains("<p>"));
}

#[test]
fn own_table_row_preserves_empty_cell() {
    let md = "| a | b | c |\n| --- | --- | --- |\n| 1 |   | 3 |";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<table>"));
    assert!(html.contains("<td></td>"));
}

#[test]
fn own_loose_task_items_render_checkbox_inside_paragraph_and_empty_task_stays_literal() {
    let md = "- [x] done\n\n- [ ] <pre>Task2</pre>\n\n- [ ] \n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<li><p><input type=\"checkbox\" checked=\"\" disabled=\"\"> done</p>"));
    assert!(html.contains("<li><p><input type=\"checkbox\" disabled=\"\"> <pre>Task2</pre></p>"));
    assert!(html.contains("<li><p>[ ]</p>"));
}

#[test]
fn own_tight_task_item_html_block_does_not_leave_trailing_newline_before_item_close() {
    let md = "- [x] <div>\n  *html*\n  </div>\n  *html*\n";
    let html = render_markdown_to_html(md, RenderOptions::default());

    assert!(html.contains("<li><input type=\"checkbox\" checked=\"\" disabled=\"\"> <div>\n<em>html</em></div>\n*html*</li>"));
}
