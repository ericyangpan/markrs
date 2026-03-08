use crate::RenderOptions;

mod ast;
mod autolink;
mod block;
mod inline;
mod lexer;
mod options;
mod parser;
mod render;
mod render_html;
mod source;
mod token;

pub(crate) fn render_markdown_to_html(input: &str, options: RenderOptions) -> String {
    render_html::render_markdown_to_html(input, options)
}
