# P0 Parser Detailed Design

Last updated: 2026-03-05

## 1. Objective

Implement a from-scratch Rust Markdown parser for `markrs` without changing the public rendering contract.

Public contract to preserve:

- `render_markdown_to_html(input: &str, options: RenderOptions) -> String`
- `RenderOptions { gfm, breaks }` semantics
- Existing own tests and marked compatibility tests remain the release gate

## 2. Non-goals (P0)

- HTML sanitize pipeline integration (P1)
- WASM runtime packaging and fallback loader (P2)
- Full AST public API stability (internal-only AST is fine in P0)

## 3. Current Baseline

- Engine now: in-house `markdown` parser modules (`src/markdown/*`) + post-process hooks
- Marked compatibility test harness already in place
- `xfail` baseline exists and is intentionally mutable during migration

Baseline freeze rule:

- Before every milestone cut, run:
  - `npm run test:compat`
  - `npm run test:own`
  - record `xfail` count delta with reason category

## 4. Architecture

Code layout target:

- `src/markdown/mod.rs`
- `src/markdown/options.rs`
- `src/markdown/source.rs`
- `src/markdown/token.rs`
- `src/markdown/ast.rs`
- `src/markdown/lexer.rs`
- `src/markdown/block.rs`
- `src/markdown/inline.rs`
- `src/markdown/render_html.rs`
- `src/markdown/autolink.rs`
- `src/markdown/tests/*` (unit-level parser tests)

Integration boundary:

- `src/lib.rs` calls `markdown::render_html(input, options)`
- CLI stays unchanged

## 5. Data Model

### 5.1 Source Span

Purpose:

- Accurate debug output
- Easier compat diff localization

Suggested structure:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}
```

### 5.2 Block AST

Suggested block node set:

- `Document { children }`
- `Paragraph { inlines }`
- `Heading { level, inlines }`
- `BlockQuote { children }`
- `List { ordered, start, tight, items }`
- `ListItem { children }`
- `CodeBlock { fenced, info, text }`
- `ThematicBreak`
- `Table { aligns, head, rows }` (GFM)
- `HtmlBlock { raw }`
- `LinkReferenceDef { label, destination, title }` (kept for resolution phase)

### 5.3 Inline AST

Suggested inline node set:

- `Text`
- `SoftBreak`
- `HardBreak`
- `CodeSpan`
- `Emphasis`
- `Strong`
- `Strikethrough` (GFM)
- `Link { href, title, children }`
- `Image { src, title, alt }`
- `HtmlInline { raw }`

## 6. Parsing Pipeline

Pipeline stages:

1. Normalize source newlines (`\r\n` -> `\n`)
2. Block parse to block AST
3. Collect and resolve link reference definitions
4. Inline parse paragraph/heading/list-item inline text
5. Render HTML
6. Post-process hooks:
   - `breaks` soft-break conversion
   - GFM plain URL/email autolink pass

## 7. Block Parser Design

### 7.1 Core strategy

- Use a line cursor with byte offsets
- Maintain container stack (`blockquote`, `list`, `list-item`)
- Implement block start precedence explicitly

### 7.2 Precedence (high-level)

Evaluation order per candidate line:

1. Container continuation checks
2. ATX heading
3. Fenced code start/continuation/end
4. Thematic break
5. List marker start/continuation
6. Blockquote marker
7. Table start (GFM only, with strict header+delimiter validation)
8. Setext heading upgrade
9. HTML block rules
10. Paragraph continuation/new paragraph

### 7.3 Critical behaviors to encode

- Lazy continuation lines in blockquotes/lists
- Tight vs loose list detection
- Indented code interaction inside lists
- Table interruption vs paragraph/setext ambiguity
- Thematic break vs list item ambiguity

## 8. Inline Parser Design

### 8.1 Scanner

- Single forward scan producing token stream
- Track:
  - delimiter runs (`*`, `_`, `~`)
  - brackets (`[`, `![`)
  - backticks
  - escapes
  - raw HTML spans

### 8.2 Delimiter algorithm

- Use delimiter stack (open/close flags + length + position)
- Resolve in reverse with CommonMark-like flanking logic
- GFM `~~` enabled only when `options.gfm == true`

### 8.3 Link and image resolution

- Inline link: `[text](dest "title")`
- Reference link:
  - full `[text][label]`
  - collapsed `[label][]`
  - shortcut `[label]`
- Unmatched brackets degrade to text

### 8.4 Autolink handling

- Keep current markrs policy:
  - plain `www.` auto-link in GFM mode
  - plain emails auto-link in GFM mode
  - skip inside `a/code/pre/script/style/textarea`
- Keep this behavior in dedicated `autolink.rs` pass so parser core remains deterministic

## 9. HTML Renderer Design

Renderer invariants:

- Deterministic tag ordering
- Stable escaping behavior for text and attributes
- No sanitizer concerns in P0

Functions:

- `escape_text`
- `escape_attr`
- `render_block`
- `render_inline`
- `render_document`

Output normalization policy:

- Keep the same general style used today to avoid noise in compat tests

## 10. Marked Compatibility Strategy

Use current harness unchanged as external oracle.

Additional guidance:

- Keep front-matter option extraction in tests (`gfm`, `breaks`)
- Keep `xfail` but classify each mismatch category:
  - block structure mismatch
  - inline delimiter mismatch
  - link/ref resolution mismatch
  - formatting-only mismatch

Reduction strategy:

1. Remove formatting-only mismatches first
2. Then block structure mismatches
3. Then inline/link edge cases

## 11. Cutover Status

During migration, the codebase is expected to converge on the new in-house parser as
the default and only active rendering path.

`markrs` legacy routing has been removed from `src/markdown/mod.rs`.
Rendering now goes through the new parser pipeline directly:
`render_html::render_markdown_to_html`.

## 12. Performance Plan

Measure before and after M3 and M5.

Benchmark sets:

- Small docs: README-scale
- Medium docs: 10-50 KB markdown
- Large docs: 100-500 KB markdown
- Pathological fixtures from `third_party/marked/test/specs/redos`

Metrics:

- Throughput (MB/s)
- p50/p95 parse time
- peak memory

Guardrail:

- No >30% regression vs baseline without explicit sign-off

## 13. Test Plan

### 13.1 Existing gates

- `tests/own_rendering.rs`
- `tests/compat_snapshot.rs`

### 13.2 New parser-focused tests

- `tests/parser_blocks.rs`
- `tests/parser_inlines.rs`
- `tests/parser_regressions.rs`

Coverage map (minimum):

- Headings: ATX + setext
- Lists: ordered/unordered, nested, tight/loose
- Blockquote nesting and lazy continuation
- Fenced and indented code interactions
- Tables and interruption rules
- Emphasis/strong/strike combinations
- Links/images/reference links
- Escapes/entities/backticks edge cases

## 14. Milestone Execution Detail

### M0

- Freeze baseline counts and categories
- Add a script to print mismatch category summary

### M1

- Create parser modules and compile path
- Implement minimal block parser + renderer
- Add engine switch and smoke tests

### M2

- Complete block constructs
- Land block-focused parser tests

### M3

- Complete inline parser and link resolution
- Land inline-focused parser tests

### M4

- Drive down `xfail` with fixture-led iterations
- Prioritize top-frequency mismatch categories

### M5

- Delete `pulldown-cmark` usage and dependency
- Remove temporary engine switch
- Finalize docs

## 15. Task Backlog (Ready for fast model)

### B1: Scaffolding

- Create module tree under `src/markdown`
- Add `RenderOptions` passthrough

### B2: AST + Renderer core

- Add AST enums/structs
- Add HTML rendering primitives with escaping

### B3: Block parser iteration

- Add cursor/container stack
- Implement heading/paragraph/hr/fenced code
- Add list/blockquote/table

### B4: Inline parser iteration

- Add tokenizer
- Add delimiter stack resolver
- Add link/image/reference resolver

### B5: Compatibility reduction loops

- Run compat
- Fix top category
- Re-run

### B6: Cutover cleanup

- Remove pulldown dependency
- Remove fallback path
- Refresh docs

## 16. Risks and Mitigations

Risk:

- List and table edge cases consume most effort

Mitigation:

- Build explicit precedence matrix tests first

Risk:

- Inline delimiter logic causes regressions

Mitigation:

- Keep tokenizer and resolver separated with focused regression tests

Risk:

- Migration causes hard-to-debug diffs

Mitigation:

- Preserve spans and add mismatch debug output keyed by span region

## 17. Decision Log

DL-001:

- Keep autolink and softbreak behavior as explicit post-process passes in P0
- Reason: reduces parser-core complexity while preserving current markrs behavior

DL-002:

- Keep parser AST internal-only in P0
- Reason: avoid freezing API before compatibility stabilizes
