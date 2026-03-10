use crate::{
    RenderOptions,
    markdown::{parser, render},
};

pub(crate) fn render_markdown_to_html(input: &str, options: RenderOptions) -> String {
    let document = parser::parse_document(input, options);
    render::render_document(&document, options)
}
