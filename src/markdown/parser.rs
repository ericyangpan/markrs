use crate::{
    RenderOptions,
    markdown::{
        block,
        lexer::{Line, LineScanner},
        options::ParserOptions,
        source::Source,
    },
};

pub(crate) fn parse_document(
    input: &str,
    options: RenderOptions,
) -> crate::markdown::ast::Document {
    let parser_options = ParserOptions::from(options);
    let source = Source::new(input);
    if source.is_empty() {
        return crate::markdown::ast::Document::Nodes(Vec::new());
    }

    let scanner = LineScanner::new(&source);
    parse_with_scanner(&scanner, parser_options)
}

fn parse_with_scanner(
    scanner: &LineScanner<'_>,
    options: ParserOptions,
) -> crate::markdown::ast::Document {
    let mut parser = BlockParser::new(scanner.as_lines(), options);
    parser.parse()
}

struct BlockParser<'a> {
    lines: &'a [Line<'a>],
    gfm: bool,
    pedantic: bool,
}

impl<'a> BlockParser<'a> {
    fn new(lines: &'a [Line<'a>], options: ParserOptions) -> Self {
        Self {
            lines,
            gfm: options.gfm,
            pedantic: options.pedantic,
        }
    }

    fn parse(&mut self) -> crate::markdown::ast::Document {
        let mut ctx = block::BlockParseContext::new();
        ctx.parse_line_slices(self.lines, self.gfm, self.pedantic)
    }
}
