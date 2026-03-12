use crate::RenderOptions;

mod ast;
mod block;
mod inline;
mod lexer;
mod options;
mod parser;
mod render;
mod render_html;
mod source;

pub(crate) fn render_markdown_to_html(input: &str, options: RenderOptions) -> String {
    render_html::render_markdown_to_html(input, options)
}

pub(crate) fn render_markdown_to_html_buf(input: &str, options: RenderOptions, buf: &mut String) {
    render_html::render_markdown_to_html_buf(input, options, buf)
}
