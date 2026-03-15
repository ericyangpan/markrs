# Architecture

This document explains where the main logic lives and how Markdown becomes HTML in `markast`.

## System Overview

The repository has three main surfaces:

- Rust library: parses Markdown and renders HTML
- Rust CLI: reads files or stdin and prints fragment or document HTML
- npm packaging: distributes the CLI as a root package plus platform-specific binary packages

## Main Entry Points

`src/main.rs`

- CLI argument parsing with `clap`
- Reads Markdown input from a file or stdin
- Calls `render_markdown_to_html(...)`
- Optionally wraps the fragment with `build_html_document(...)`

`src/lib.rs`

- Public library surface
- Defines `RenderOptions`
- Defines theme file format and HTML document builder
- Routes rendering requests into `src/markdown`

## Rendering Pipeline

Current high-level flow:

1. `src/main.rs` or library callers pass Markdown plus `RenderOptions`
2. `src/markdown/render_html.rs` calls `parser::parse_document(...)`
3. `src/markdown/parser.rs` normalizes access to line-based parsing
4. `src/markdown/block.rs` builds the block-level document tree
5. `src/markdown/inline.rs` resolves inline syntax inside block content
6. `src/markdown/render.rs` converts the AST into HTML
7. `src/markdown/autolink.rs` applies post-render autolink processing

The public API contract is preserved through `render_markdown_to_html(input, options) -> String`.

## Module Map

`src/markdown/ast.rs`

- Internal document model used by the parser and renderer

`src/markdown/source.rs`

- Source abstraction for normalized line access

`src/markdown/lexer.rs`

- Low-level line scanning helpers used by parsing

`src/markdown/options.rs`

- Internal parser option mapping from public `RenderOptions`

`src/markdown/parser.rs`

- Parser entrypoint and line-scanner orchestration

`src/markdown/block.rs`

- Block parsing logic
- Lists, blockquotes, code blocks, headings, tables, HTML blocks, references

`src/markdown/inline.rs`

- Inline parsing logic
- Links, emphasis, images, code spans, raw HTML, task markers

`src/markdown/render.rs`

- HTML renderer for the internal AST

`src/markdown/render_html.rs`

- Top-level render coordinator

`src/markdown/autolink.rs`

- GFM-style post-processing for plain URL and email autolinks

## Tests by Responsibility

`tests/own_rendering.rs`

- Product-level assertions for `markast` behavior

`tests/parser_blocks.rs`

- Focused block parser regressions

`tests/parser_inlines.rs`

- Focused inline parser regressions

`tests/parser_regressions.rs`

- Fixture-backed targeted regressions

`tests/compat_snapshot.rs`

- Compares `markast` output to vendored `marked` fixture snapshots

`tests/compat_runtime.rs`

- Compares `markast` output to the current vendored `marked` npm runtime

## npm and Release Layout

Root `package.json`:

- exposes the `markast` binary through `bin/markast.js`
- defines developer commands
- depends on platform packages through `optionalDependencies`

`npm/*` packages:

- each package contains one platform-specific binary
- versions must stay aligned with the root package

`.github/workflows/ci.yml`:

- checks npm version sync
- runs the strict Rust gate

`.github/workflows/release.yml`:

- rebuilds platform binaries
- stages platform packages
- publishes platform packages and then the root npm package

## Where To Change Things

If the task is about CLI flags or input handling, start in `src/main.rs`.

If the task is about public rendering options or document themes, start in `src/lib.rs`.

If the task changes Markdown semantics, start in `src/markdown/*` and the focused parser tests.

If the task changes compatibility expectations, inspect `tests/compat_*`, `tests/compat/*.yaml`, and `scripts/render-marked-runtime.mjs`.
