# P0 Parser Rebuild Plan

Last updated: 2026-03-12

## Goal

Build a Rust Markdown parser from scratch and remove `pulldown-cmark` from `markrs`.

Detailed design:

- `docs/p0-parser-design.md`

## Scope

- In scope:
  - Replace current parse engine used by `render_markdown_to_html`.
  - Keep current HTML output contract and `RenderOptions` behavior (`gfm`, `breaks`).
  - Keep passing own tests and marked compatibility tests.
- Out of scope for P0:
  - Sanitization pipeline (P1).
  - WASM packaging/runtime fallback (P2).

## Compatibility Contract

- Must preserve:
  - `render_markdown_to_html(input, options) -> String`
  - GFM toggle semantics (`options.gfm`)
  - line-break toggle semantics (`options.breaks`)
- Must not regress:
  - `npm run test:own`
  - `npm run test:compat`
- `tests/compat/xfail.yaml` is allowed during migration, but trend must be downward.

## Autonomous Execution Loop

This is the standing work agreement for parser/runtime compatibility work. It takes precedence over the older dated execution notes later in this file.

Current baseline (2026-03-12 local report):

- current-runtime compatibility: `1402` passed, `83` gaps
- vendored snapshot compatibility: `1404` passed, `81` gaps
- isolated benchmark reference:
  - `Comparable Corpus`: `1.57x` vs `marked`
  - `Marked Fixtures`: `1.46x` vs `marked`

Default aggressive target:

- reduce `tests/compat/runtime_xfail.yaml` from `83` to `0` before asking for a new direction
- keep `tests/compat/xfail.yaml` moving downward when the change also helps runtime parity, or when the remaining delta is intentionally classified as snapshot-only
- preserve isolated benchmark guardrails while fixing compatibility:
  - `Comparable Corpus >= 1.25x` vs `marked`
  - `Marked Fixtures >= 1.00x` vs `marked`

Default batch loop:

1. Pick the highest-yield cluster from `tests/compat/runtime_xfail.yaml`.
2. Make the narrowest parser or renderer change that removes a real mismatch category.
3. Add or promote focused regression coverage near the affected layer.
4. Run focused tests, then `npm run check:strict`, then `npm run test:compat`.
5. If hot paths changed, run `npm run bench` in isolation.
6. Update `tests/compat/runtime_xfail.yaml` and `tests/compat/xfail.yaml` only when the behavior change is intentional and verified.
7. Commit the batch and continue to the next cluster without waiting for approval.

Escalate only when:

- a fix requires changing the public API, CLI behavior, package contract, or documented default semantics
- a fix requires editing `third_party/marked/*` or changing the vendored/current `marked` version target
- runtime and snapshot targets require conflicting behavior that the current harness cannot represent cleanly
- three consecutive well-scoped batches produce no net reduction in runtime gaps
- a correctness fix would push isolated benchmark results below the guardrail floor

## Architecture Split

### 1) Syntax Model

- `src/markdown/ast.rs`
- Define block/inline nodes with source spans.
- Keep spans for better diff/debug output in compat failures.

### 2) Lexer / Scanner

- `src/markdown/lexer.rs`
- Line scanner + inline token scanner utilities.
- Deterministic behavior; no regex-heavy backtracking in hot path.

### 3) Block Parser

- `src/markdown/block.rs`
- Parse:
  - paragraph
  - ATX/setext headings
  - blockquote
  - ordered/unordered list (+ nesting/tight-loose)
  - fenced/indented code
  - thematic break
  - table (gfm)
  - html block (pass-through rules)
  - reference definitions

### 4) Inline Parser

- `src/markdown/inline.rs`
- Parse:
  - emphasis/strong/strikethrough
  - codespan
  - links/images/reflinks/autolinks
  - escapes/entities
  - raw html inline
  - tasklist checkbox marker handling (gfm)

### 5) HTML Renderer

- `src/markdown/html.rs`
- Render AST to HTML.
- Keep current post-process hooks:
  - softbreak-to-`<br>` by option
  - plain URL/email autolink policy compatible with marked tests

### 6) Public API Bridge

- `src/markdown/mod.rs`
- `src/lib.rs`
- `render_markdown_to_html` routes to the new parser module.

## Milestones

### M0: Freeze Baseline

- Lock current compat baseline and snapshot stats.
- Add script output: total cases / xfail / recovered / new failures.

Exit criteria:

- Baseline reproducible locally and in CI.

### M1: Skeleton + Minimal Pipeline

- Add parser module layout and compile-only scaffolding.
- Implement `paragraph`, `heading`, `code fence`, `hr`.

Exit criteria:

- New parser path is the runtime default rendering route.
- Own tests compile and run.

Status: completed (default render route switched to `render_markdown_to_html` in `src/markdown/mod.rs`; old `markrs` branch removed).

### M2: Block Completeness

- Implement list/blockquote nesting, indented code, references, table (gfm).
- Add focused unit tests per construct.

Exit criteria:

- No catastrophic parse failures on marked fixtures.

Status: in progress (new block AST + block table/list/blockquote/code paths are mounted behind line-driven block parse; nested edge-cases and interruption rules still pending).

### M3: Inline Completeness

- Implement delimiter stack for emphasis/strong/strike.
- Implement links/images/reference links and edge cases.

Exit criteria:

- Majority of remaining xfails are known HTML formatting diffs or rare edge rules.

Status: in progress (inliner now route is objectized and reference-aware; full delimiter-stack rewrite is pending).

### M4: Marked Edge Behavior

- Address high-value incompatibilities:
  - list tight/loose and interruption rules
  - table vs setext ambiguity
  - autolink quotes and punctuation boundaries
  - pedantic/front-matter-controlled paths used by marked fixtures

Exit criteria:

- `xfail` count significantly reduced from current baseline.

Status: in progress (compat baseline still needs staged reduction by category).

### M5: Cutover

- Remove `pulldown-cmark` dependency.
- Make new parser default.

Exit criteria:

- `Cargo.toml` has no `pulldown-cmark`.
- `npm run test:own` and `npm run test:compat` pass.

Status: parser dependency cutover is structurally complete; next step is evidence pass for `test:own`/`test:compat` after staged fixes.

## Execution Update (2026-03-06)

Current baseline snapshot:

- marked compat suite: `1485` total cases, `614` baseline `xfail`
- current top CommonMark failure sections:
  - `Emphasis and strong emphasis`: `130`
  - `Links`: `68`
  - `List items`: `50`
  - `Lists`: `38`
  - `Images`: `32`
  - `Link reference definitions`: `30`
- current gate gaps before parser rewrites:
  - `cargo test --all-targets` fails in inline unit assertions
  - `npm run test:own` has assertion noise mixed with parser behavior checks
  - planned focused parser test files do not exist yet

Next execution order:

### E0: Gate Repair

- Fix inline unit test compile failures.
- Remove or correct own-test assertions that are checking unstable formatting instead of parser behavior.
- Exit criteria:
  - `cargo test --all-targets` runs
  - `npm run test:own` is green

### E1: Focused Regression Net

- Add `tests/parser_blocks.rs`, `tests/parser_inlines.rs`, `tests/parser_regressions.rs`.
- Seed them with:
  - passing focused coverage for parser contracts already expected to work
  - ignored compat-backed regression cases for known gaps by category
- Exit criteria:
  - new parser changes can be validated without relying only on the full compat harness

### E2: Inline Delimiter / Link-Ref Rewrite

- Highest-yield batch; do this before chasing more fixture-specific xfails.
- Replace greedy delimiter matching with delimiter-stack behavior for `*`, `_`, `~`.
- Unify reference-link normalization with block definition parsing and continuation behavior.
- Target compat categories:
  - emphasis / strong emphasis
  - links / images
  - link reference definitions

### E3: List Model and Paragraph Interruption

- Add explicit list semantics needed by render:
  - `tight` / `loose`
  - marker-aware behavior where necessary
- Remove renderer-side guessing for list paragraph flattening.
- Tighten paragraph interruption rules around list markers.

### E4: Table / Setext / HTML / Blockquote Cleanup

- Fix table row continuation and column normalization.
- Fix table vs setext precedence.
- Narrow HTML block detection so inline HTML is not promoted too early.
- Preserve blockquote indentation needed for nested code/list structure.

Execution notes:

- Do not use `npm run test:compat:update-xfail` as a routine workflow step.
- Every compat reduction batch must include:
  - one focused parser test
  - one compat delta check
  - explicit removal of recovered ids instead of blind baseline refresh

## Rebucket Update (2026-03-06, after E3/E4 pass)

Current baseline snapshot:

- marked compat suite: `1485` total cases, `420` baseline `xfail`
- current focused regression status:
  - `table_vs_setext`: promoted to enforced regression test and passing
  - `nested_blockquote_in_list`: promoted to enforced regression test and passing
  - `incorrectly_formatted_list_and_hr`: still ignored; remaining mismatch is now mostly pretty-HTML / normalization noise, not a high-value parser semantic gap

Remaining `xfail` concentration by bucket:

- CommonMark / GFM sections:
  - `Links`: `62`
  - `List items`: `36`
  - `Images`: `32`
  - `Link reference definitions`: `30`
  - `Autolinks`: `26`
  - `Entity and numeric character references`: `22`
  - `Raw HTML`: `22`
  - `Lists`: `20`
  - `HTML blocks`: `16`
  - `Setext headings`: `14`
  - `Block quotes`: `14`
- non-CommonMark buckets:
  - `new/*`: `41`
  - `original/*`: `10`
  - `gfm.0.29`: `15`

Implication:

- The next highest-yield parser batch is no longer table/list cleanup.
- The next batch should target the link family as one cluster:
  - links
  - images
  - link reference definitions
  - autolinks
- This cluster is large enough that it should be treated as a dedicated execution stage, not mixed with unrelated block cleanup.

Execution change:

### E5: Link / Image / RefDef / Autolink Cluster

- Scope:
  - remaining inline link destination edge cases
  - reference definition lookup / normalization mismatches
  - image title / label edge cases
  - autolink boundaries and entity interaction
- Guardrails:
  - do not refresh `xfail` until the whole link-family batch is complete
  - add 3-5 focused parser/regression tests first, then change parser behavior
  - keep `incorrectly_formatted_list_and_hr` parked unless a real semantic gap is identified

Status update (2026-03-06, E5 batch 1 complete):

- compat baseline moved from `420` to `357`
- completed in this batch:
  - multiline reference-definition titles
  - Unicode case-folded reference labels
  - reference-image resolution with flattened `alt` text
  - generic scheme autolinks
  - angle-autolink boundary fix for spaced `< ... >` inputs
- promoted regressions now enforced and passing:
  - CommonMark `example-196`
  - CommonMark `example-206`
  - CommonMark `example-573`
  - CommonMark `example-598`
- next E5 focus:
  - remaining link destination / entity interaction edge cases
  - remaining image/title variants not covered by recovered CommonMark cases

Status update (2026-03-06, E5 batch 2 complete):

- compat baseline moved from `357` to `306`
- completed in this batch:
  - inline link destination parser rewrite for angle / bare destinations
  - destination normalization for escapes, entity decoding, and percent-encoding
  - title parsing for quoted and parenthesized variants
  - block/container-aware reference-definition prescan rules
  - `pedantic` inline fallback for unclosed angle destinations used by marked fixtures
- promoted regressions now enforced and passing:
  - CommonMark `example-202`
  - CommonMark `example-213`
  - CommonMark `example-218`
  - CommonMark `example-503`
  - CommonMark `example-505`
  - CommonMark `example-609`
  - `new/link_lt`
  - `new/def_blocks`
- remaining deferred gap is still:
  - `incorrectly_formatted_list_and_hr`

Status update (2026-03-07, E5 batch 3 complete):

- compat baseline moved from `306` to `296`
- completed in this batch:
  - reference-label case folding for `ẞ/ß -> ss`
  - first-definition-wins semantics for duplicate reference definitions
  - non-pedantic restriction that full/collapsed reference links do not cross line breaks between labels
  - pedantic-only allowance for reference-style links that span a line break between labels
- promoted regressions now enforced and passing:
  - CommonMark `example-540`
  - CommonMark `example-543`
  - CommonMark `example-544`
  - CommonMark `example-556`
- remaining high-yield E5 work is now concentrated in:
  - nested/illegal link fallback behavior
  - remaining image/link label edge cases

Status update (2026-03-07, E5 batch 4 complete):

- compat baseline moved from `296` to `268`
- completed in this batch:
  - outer link/reference-link fallback when the parsed label contains an inner link
  - raw HTML and autolink skipping during bracket matching, so `]` inside those spans no longer closes a link label
  - non-pedantic restriction that full reference links do not bridge a space between the two bracketed labels
  - percent-encoding of `[` and `]` in normalized destinations for recovered autolink-in-label cases
- promoted regressions now enforced and passing:
  - CommonMark `example-518`
  - CommonMark `example-520`
  - CommonMark `example-524`
  - CommonMark `example-526`
  - CommonMark `example-532`
  - CommonMark `example-542`
- remaining high-yield E5 work is now concentrated in:
  - the rest of the illegal/nested link fallback family
  - residual autolink/entity/image tail cases outside the recovered cluster

Deferred cleanup:

- `incorrectly_formatted_list_and_hr` should only be revisited if:
  - compat normalization is intentionally expanded, or
  - a real block semantic mismatch reappears after future parser changes

Status update (2026-03-07, E6 list/blockquote indentation batch complete):

- compat baseline moved from `268` to `201`
- completed in this batch:
  - list-item `content_indent` now tracks marker padding width instead of using a fixed continuation guess
  - list item collection now distinguishes sibling-vs-nested markers by indentation level instead of exact raw indent equality
  - partial-tab continuation stripping is preserved for nested list structure, so tab-indented continuations no longer collapse incorrectly
  - blockquote marker stripping now removes exactly one marker padding character (`space` or `tab`) instead of trimming all leading whitespace
  - fenced code blocks now remove opening-fence indent from content lines, fixing list-contained tab-indented fence payloads
- promoted regressions now enforced and passing:
  - CommonMark `example-259`
  - `new/tab_after_blockquote`
  - focused parser coverage for quoted list continuation and list-contained fenced-code indent normalization
- notable recovered clusters:
  - multiple CommonMark/GFM list-item and blockquote examples in the `250s`, `270s`, `280s`, and `310s`
  - `original/blockquotes_with_code_blocks`
  - `new/list_wrong_indent`
  - `new/tricky_list`
- next high-yield cluster after this batch:
  - remaining `Links / Images / Entity` tail cases, or
  - a deliberate return to the parked `incorrectly_formatted_list_and_hr` gap if block semantics become the priority again

Status update (2026-03-07, E7 HTML/raw-HTML batch complete):

- compat baseline moved from `201` to `159`
- completed in this batch:
  - HTML block start detection now distinguishes paragraph-interrupting block forms from generic inline-tag forms
  - HTML block parsing now keeps block tags open until the terminating blank line and supports comment / processing-instruction / declaration / CDATA forms
  - raw HTML inline parsing now accepts processing instructions, declarations, and CDATA in addition to tag-like spans
  - HTML tag parsing now handles boolean attributes followed by additional attributes without prematurely rejecting the span
- promoted regressions now enforced and passing:
  - CommonMark `example-151`
  - CommonMark `example-180`
  - CommonMark `example-185`
  - CommonMark `example-627`
  - CommonMark `example-628`
  - CommonMark `example-629`
- notable recovered clusters:
  - multiple CommonMark/GFM HTML block examples in the `140s`, `160s`, and `180s`
  - multiple CommonMark/GFM raw HTML examples in the `620s`
  - hard-line-break tail examples `642` and `643` recovered as part of the same parsing cleanup
- remaining high-yield work after this batch:
  - residual `Links / Images / Entity` tail cases
  - numeric/entity reference normalization edges
  - the parked `incorrectly_formatted_list_and_hr` regression if a real block semantic gap remains after later cleanup

Status update (2026-03-07, E8 entity/entity-reference batch complete):

- compat baseline moved from `159` to `142`
- completed in this batch:
  - added a shared HTML entity parser so inline text, reference metadata, and fenced-code info strings all use the same decode path
  - expanded named entity coverage for the remaining compat corpus, including multi-code-point entities such as `&ngE;`
  - numeric character references now replace invalid scalar values such as `&#0;` with `U+FFFD` instead of leaving the source literal behind
  - inline entity decoding now emits literal text nodes, so decoded `*`, tabs, and newlines no longer accidentally trigger emphasis, list, or block parsing
- promoted regressions now enforced and passing:
  - CommonMark `example-25`
  - CommonMark `example-26`
  - CommonMark `example-34`
  - CommonMark `example-37`
  - CommonMark `example-38`
  - CommonMark `example-39`
- notable recovered clusters:
  - CommonMark/GFM `Entity and numeric character references` examples `25`, `26`, `27`, `34`, `37`, `38`, `39`, `40`, and `41`
  - `original/amps_and_angles_encoding`
- remaining high-yield work after this batch:
  - `Setext headings` and related block-interruption fixtures
  - residual `Lists / Tabs / Hard line breaks` tail cases
  - the parked `incorrectly_formatted_list_and_hr` regression if a real block semantic gap remains after later cleanup

Status update (2026-03-07, E9 setext/blockquote boundary batch complete):

- compat baseline moved from `142` to `122`
- completed in this batch:
  - setext heading content now strips paragraph-continuation indent and trailing horizontal whitespace before inline parsing
  - single-character setext underlines are accepted, matching marked for lone `=` / `-` forms
  - unquoted thematic-break lines no longer remain inside lazy blockquotes, while short `==` / `--` lazy continuations are preserved
  - paragraph continuation lines are normalized only when appended, so block-interruption checks still see the original raw line shape
- promoted regressions now enforced and passing:
  - CommonMark `example-82`
  - CommonMark `example-83`
  - CommonMark `example-89`
  - CommonMark `example-92`
  - CommonMark `example-101`
- notable recovered clusters:
  - CommonMark/GFM `Setext headings` examples `82`, `83`, `84`, `87`, `89`, `92`, and `101`
  - CommonMark/GFM paragraph/blockquote side cases `49`, `70`, `113`, and `234`
  - `new/list_item_empty`
- remaining high-yield work after this batch:
  - `new/blockquote_setext` paragraph softbreak-vs-space normalization
  - `new/pedantic_heading` and `new/pedantic_heading_interrupts_paragraph`
  - table-tail rows that currently escape into setext (`lheading_following_*`, `inlinecode_following_*`, `strong_following_*`, `text_following_*`)

Status update (2026-03-07, E10 table-tail continuation batch complete):

- compat baseline moved from `122` to `112`
- completed in this batch:
  - table parsing now supports marked-style implicit tail rows after a valid table body, so plain text and inline-only lines can continue the table as a first-column cell with remaining cells padded empty
  - implicit tail rows stop cleanly before block-start lines such as ATX headings, blockquotes, fences, lists, and paragraph-interrupting HTML blocks
  - table blank-line detection now uses Markdown block whitespace rules (`space`/`tab` only), so `NBSP` tail rows are preserved and collapse to empty cells instead of ending the table
- promoted regressions now enforced and passing:
  - `new/lheading_following_table`
  - `new/inlinecode_following_tables`
  - `new/text_following_tables`
  - `new/nbsp_following_tables`
- notable recovered clusters:
  - `new/lheading_following_table`
  - `new/lheading_following_nptable`
  - `new/inlinecode_following_tables`
  - `new/inlinecode_following_nptables`
  - `new/strong_following_tables`
  - `new/strong_following_nptables`
  - `new/text_following_tables`
  - `new/text_following_nptables`
  - `new/nbsp_following_tables`
  - GFM spec `example-5`
- remaining high-yield work after this batch:
  - `new/blockquote_setext`
  - `new/pedantic_heading` and `new/pedantic_heading_interrupts_paragraph`
  - `Lists / Tabs / fenced-code tail` cases such as `list_item_tabs`, `list_item_text`, `list_loose_tasks`, and `fences_breaking_paragraphs`

Status update (2026-03-07, E11 list-tabs batch complete):

- compat baseline moved from `112` to `107`
- completed in this batch:
  - list marker padding now measures tabs from the real marker column instead of column zero, so ordered and unordered items using `\t` after the marker no longer over-indent their continuation threshold
  - tab-indented continuation paragraphs stay inside the owning list item instead of being misparsed as indented code blocks
  - tab-indented nested list items stay nested across multiple levels
  - loose task items now render checkboxes inside the first paragraph, and empty task markers such as `[ ]` fall back to literal text
  - indented code detection and stripping now honor tab-expanded leading columns, recovering the CommonMark tab semantics that previously stayed xfailed
- promoted regressions now enforced and passing:
  - `new/list_item_tabs`
  - `new/list_loose_tasks`
- notable recovered cases:
  - `new/list_item_tabs`
  - `new/list_loose_tasks`
  - `original/inline_html_simple`
  - CommonMark/GFM `example-2`
- remaining high-yield work after this batch:
  - `new/list_item_text`
  - `new/list_align_pedantic`
  - `new/paragraph-after-list-item`
  - remaining block/list interaction tails such as `fences_breaking_paragraphs` and `tasklist_blocks`

Status update (2026-03-07, E12 pedantic-list and tight-list-heading batch complete):

- compat baseline moved from `107` to `106`
- completed in this batch:
  - pedantic list-item collection now keeps outdented continuation text inside a non-zero-indented parent item after nested sublists instead of ejecting it to top level
  - tight list items now render an inline-text paragraph followed by a heading with a single separating space, matching marked's `list_code_header` HTML shape
- promoted regressions now enforced and passing:
  - `new/list_code_header`
- notable observations from this batch:
  - `new/list_item_text` is no longer a structural parser mismatch; the remaining diff is a marked pretty-HTML trailing space before `</li>`, so it stays in baseline for now
- remaining high-yield work after this batch:
  - code-block trailing-newline mismatches such as `code_block_no_ending_newline`, `paragraph-after-list-item`, `indented_details`, and `fences_breaking_paragraphs`
  - pedantic list alignment cases such as `list_align_pedantic`

Status update (2026-03-07, E13 pedantic-heading batch complete):

- compat baseline moved from `106` to `103`
- completed in this batch:
  - ATX heading parsing is now pedantic-aware: `#h1` style headings are accepted without a space, leading indentation is rejected in pedantic mode, and trailing closing `#` markers are stripped using marked-compatible pedantic rules
  - pedantic paragraph parsing now yields before a following setext underline candidate, so `pedantic_heading_interrupts_paragraph` no longer absorbs the previous paragraph line
  - the same pedantic heading rules now apply in non-GFM mode, recovering `nogfm_hashtag`
- promoted regressions now enforced and passing:
  - `new/pedantic_heading`
  - `new/pedantic_heading_interrupts_paragraph`
  - `new/nogfm_hashtag`
- remaining high-yield work after this batch:
  - code-block trailing-newline mismatches such as `code_block_no_ending_newline`, `paragraph-after-list-item`, `indented_details`, and `fences_breaking_paragraphs`
  - pedantic list alignment in `list_align_pedantic`

Status update (2026-03-07, E14 table cell and header validation batch complete):

- compat baseline moved from `103` to `98`
- completed in this batch:
  - table cell splitting is now aware of escaped pipes and backtick code spans, so `\|` and `` `|` `` no longer break GFM table cell boundaries
  - table headers now require an exact column-count match with the delimiter row instead of truncating mismatched headers into a table
  - header-only tables now omit an empty `<tbody>`, matching marked's HTML shape
- promoted regressions now enforced and passing:
  - `new/table_cells`
  - `test/specs/gfm/gfm.0.29.json#example-3`
  - `test/specs/gfm/gfm.0.29.json#example-6`
  - `test/specs/gfm/gfm.0.29.json#example-8`
- notable recovered cases:
  - `new/table_cells`
  - `new/tab_newline`
  - `test/specs/gfm/gfm.0.29.json#example-3`
  - `test/specs/gfm/gfm.0.29.json#example-6`
  - `test/specs/gfm/gfm.0.29.json#example-8`
- remaining high-yield work after this batch:
  - html/setext interruption edges such as `setext_no_blankline`
  - residual table/block paragraph tails such as `indented_tables`
  - code-block/list boundary cases such as `paragraph-after-list-item` and `fences_breaking_paragraphs`

Status update (2026-03-07, E15 html-vs-setext interruption batch complete):

- compat baseline moved from `98` to `97`
- completed in this batch:
  - setext heading collection now bails out when the current line is already a block-HTML opener, so `<html>\n=` stays an HTML block instead of turning into a heading
- promoted regressions now enforced and passing:
  - `new/setext_no_blankline`
- notable recovered cases:
  - `new/setext_no_blankline`
- remaining high-yield work after this batch:
  - residual block parser gaps in `indented_tables`, `paragraph-after-list-item`, `fences_breaking_paragraphs`, and `list_align_pedantic`
  - pretty-HTML-only tails such as `list_item_text` should stay de-prioritized unless they uncover a structural parser bug

Status update (2026-03-08, E16 pedantic-list normalization and indented-code blank-line batch complete):

- compat baseline moved from `97` to `94`
- completed in this batch:
  - pedantic list continuation lines now normalize nesting with a marked-style leading-space rewrite before item collection and content stripping, which fixes the structural shape of deeply staggered pedantic sublists
  - pedantic list-item collection now keeps blank-line-separated nested list markers inside an indented parent item instead of ejecting them as top-level blocks
  - indented code blocks now keep whitespace-only blank lines inside the same code block instead of splitting into multiple code blocks
- promoted regressions now enforced and passing:
  - `new/list_align_pedantic`
  - `new/whiltespace_lines`
  - `commonmark example 111`
- notable recovered cases:
  - `new/whiltespace_lines`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-111`
  - `test/specs/gfm/commonmark.0.31.2.json#example-111`
- notable observations from this batch:
  - `new/list_align_pedantic` is now structurally aligned with marked, but the remaining compat diff is still renderer whitespace around nested `<ul>` boundaries
  - `new/list_item_text` has been reduced back to a formatting-only tail rather than a list-boundary parser bug
- remaining high-yield work after this batch:
  - code-block trailing-newline mismatches such as `code_block_no_ending_newline`, `paragraph-after-list-item`, and `indented_details`
  - paragraph softbreak formatting mismatches such as `blockquote_setext` and `emphasis_extra tests`
  - legacy fixture oddities such as `em_and_reflinks`, where the vendored expected HTML now differs from current upstream marked behavior

Status update (2026-03-08, E17 fenced-info normalization and numeric-entity validation batch complete):

- compat baseline moved from `94` to `86`
- completed in this batch:
  - fenced code blocks now derive the `language-...` class from only the first whitespace-delimited info token, matching CommonMark instead of treating the full info string as the class name
  - fenced code info tokens now honor backslash escapes before ASCII punctuation when deriving the language class, so ```` ``` foo\+bar ```` renders `language-foo+bar`
  - numeric entities longer than 7 decimal digits or 6 hex digits are now rejected as invalid and left literal instead of decoding to `U+FFFD`
- promoted regressions now enforced and passing:
  - `commonmark example 24`
  - `commonmark example 28`
  - `commonmark example 143`
  - `commonmark example 146`
- notable recovered cases:
  - `test/specs/commonmark/commonmark.0.31.2.json#example-24`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-28`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-143`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-146`
  - the corresponding GFM CommonMark mirror cases
- remaining high-yield work after this batch:
  - container-boundary parser gaps such as `paragraph-after-list-item`, `fences_breaking_paragraphs`, and `indented_details`
  - renderer-format-only tails such as `list_align_pedantic` and `list_item_text`
  - the still-ignored compat gap `incorrectly_formatted_list_and_hr`

Status update (2026-03-08, E18 blockquote lazy-continuation state batch complete):

- compat baseline moved from `86` to `82`
- completed in this batch:
  - nested blockquote parsing now strips inherited lazy-prefix sentinels before re-parsing, so lazy continuation markers no longer leak into descendant paragraphs or fenced-code contents
  - blockquotes no longer accept unquoted lazy continuation after a quoted blank line or while a quoted fenced code block is open
  - indented unquoted lazy lines now continue a quoted paragraph instead of being misclassified as top-level indented code
- promoted regressions now enforced and passing:
  - `commonmark example 237`
  - `commonmark example 238`
  - `commonmark example 249`
  - `commonmark example 250`
- notable recovered cases:
  - `test/specs/commonmark/commonmark.0.31.2.json#example-237`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-238`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-249`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-250`
  - the corresponding GFM CommonMark mirror cases
- remaining high-yield work after this batch:
  - container/tab/list indentation cases such as `commonmark examples 5, 6, 7, 278, 280, 304, 307, 312, 317, 319`
  - remaining new-fixture container gaps such as `paragraph-after-list-item`, `fences_breaking_paragraphs`, `tasklist_blocks`, and `indented_tables`
  - renderer-format-only tails such as `list_align_pedantic` and `list_item_text`

## File Plan

- New:
  - `src/markdown/mod.rs`
  - `src/markdown/ast.rs`
  - `src/markdown/lexer.rs`
  - `src/markdown/block.rs`
  - `src/markdown/inline.rs`
  - `src/markdown/html.rs`
  - `tests/parser_blocks.rs`
  - `tests/parser_inlines.rs`
  - `tests/parser_regressions.rs`
- Update:
  - `src/lib.rs`
  - `Cargo.toml`
  - `README.md`

## Risk Register

- Risk: list and table interruption rules are the largest compat gap.
  - Mitigation: implement explicit precedence matrix + fixture-driven tests.
- Risk: emphasis/link delimiter interactions can explode in complexity.
  - Mitigation: single-pass delimiter stack design, no ad-hoc regex fallback.
- Risk: performance regression.
  - Mitigation: add micro-bench fixture set before cutover.

## Execution Rules

- Every parser behavior change must include:
  - one focused unit test
  - one compat impact check
- No blind baseline refresh:
  - if `xfail` changes, include reason category in commit message.

Status update (2026-03-08, E19 container-indent and paragraph-boundary batch complete):

- compat baseline moved from `82` to `58`
- completed in this batch:
  - tab-expanded list and blockquote continuation indentation now normalizes residual columns correctly instead of leaking raw tab overhang into nested container parsing
  - ordered-list paragraph interruption now follows CommonMark more closely: only `1.` can interrupt a paragraph, and empty items no longer absorb following blocks after a blank line
  - paragraph continuation lines now strip container indent consistently and trim terminal hard-break spaces before inline parsing
  - blockquote lazy continuation now stops after quoted indented code, preventing unquoted lines from leaking into quoted code-adjacent paragraphs
- promoted regressions now enforced and passing:
  - `commonmark example 5`
  - `commonmark example 6`
  - `commonmark example 7`
  - `commonmark example 223`
  - `commonmark example 226`
  - `commonmark example 236`
  - `commonmark example 278`
  - `commonmark example 280`
  - `commonmark example 304`
  - `commonmark example 636`
  - `commonmark example 637`
  - `commonmark example 645`
- notable recovered cases:
  - `new/whiltespace_lines.md`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-5`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-6`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-7`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-223`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-226`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-236`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-278`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-280`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-304`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-636`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-637`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-645`
  - the corresponding GFM CommonMark mirror cases
- remaining high-yield work after this batch:
  - list/container looseness edges such as `commonmark examples 109, 307, 312, 317, 318, 319`
  - remaining new-fixture block gaps such as `paragraph-after-list-item`, `fences_breaking_paragraphs`, and `indented_details`
  - renderer-format-only tails such as `list_align_pedantic` and `list_item_text`

Status update (2026-03-08, E20 list looseness and nested-container boundary batch complete):

- compat baseline moved from `58` to `46`
- completed in this batch:
  - blockquotes can now lazily continue through quoted list items whose first child still owns paragraph content, fixing list-in-quote continuation without reopening top-level paragraphs
  - tight-list rendering now flattens paragraph children anywhere in the item instead of only flattening the first paragraph, which matches CommonMark list items containing headings or nested blocks
  - list looseness now distinguishes blank lines that start a direct child block from blank lines that only live inside a nested list or fenced code block
  - blank lines before nested lists or reference definitions now loosen the parent item, while blank lines contained entirely inside nested descendants no longer loosen outer lists
- promoted regressions now enforced and passing:
  - `commonmark example 109`
  - `commonmark example 292`
  - `commonmark example 300`
  - `commonmark example 307`
  - `commonmark example 317`
  - `commonmark example 318`
  - `commonmark example 319`
- notable recovered cases:
  - `test/specs/commonmark/commonmark.0.31.2.json#example-109`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-292`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-300`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-307`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-317`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-318`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-319`
  - the corresponding GFM CommonMark mirror cases
- remaining high-yield work after this batch:
  - under-indented nested-list rejection such as `commonmark example 312`
  - new-fixture parser gaps such as `paragraph-after-list-item`, `tasklist_blocks`, `indented_tables`, and `fences_breaking_paragraphs`
  - inline / renderer tails such as `example-20`, `example-346`, `example-593`, `example-603`, and the remaining GFM table/tasklist fixtures

Status update (2026-03-08, E21 autolink and disallowed-HTML batch complete):

- compat baseline moved from `46` to `36`
- completed in this batch:
  - GFM bare autolink post-processing no longer relies on generic `linkify` behavior for marked-specific edge cases
  - bare `www.` / scheme / scheme-email autolinks now match marked's termination rules more closely for trailing punctuation, entity-like suffixes, and `xmpp:` path handling
  - angle autolink destinations now preserve literal backslashes and percent-encode backticks / brackets instead of reusing reference-link destination normalization
  - GFM disallowed raw HTML tags are now escaped selectively at render time without clobbering allowed raw tags in the same fragment
- promoted regressions now enforced and passing:
  - `commonmark example 20`
  - `commonmark example 346`
  - `commonmark example 603`
  - `gfm example 18`
  - `gfm example 19`
  - `gfm example 20`
  - `gfm example 25`
  - `gfm example 27`
  - `gfm example 28`
- notable recovered cases:
  - `test/specs/commonmark/commonmark.0.31.2.json#example-20`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-346`
  - `test/specs/commonmark/commonmark.0.31.2.json#example-603`
  - `test/specs/gfm/gfm.0.29.json#example-18`
  - `test/specs/gfm/gfm.0.29.json#example-19`
  - `test/specs/gfm/gfm.0.29.json#example-20`
  - `test/specs/gfm/gfm.0.29.json#example-24`
  - `test/specs/gfm/gfm.0.29.json#example-25`
  - `test/specs/gfm/gfm.0.29.json#example-26`
  - `test/specs/gfm/gfm.0.29.json#example-27`
  - `test/specs/gfm/gfm.0.29.json#example-28`
- remaining high-yield work after this batch:
  - inline emphasis/reference tails such as `new/em_and_reflinks`, `new/em_list_links`, `new/emoji_strikethrough`, `new/emphasis_extra tests`, and `new/unicode_punctuation`
  - pedantic link-title/reference behavior such as `original/links_inline_style`, `original/literal_quotes_in_titles`, and `commonmark example 593`
  - block/container tails such as `paragraph-after-list-item`, `tasklist_blocks`, `indented_tables`, `fences_breaking_paragraphs`, and `toplevel_paragraphs`
  - low-value compat noise still mixed into the harness via `gfm/commonmark` mirror cases such as `170-178`, `602`, `606`, `608`, `611`, and `612`; these should stay xfailed unless we split mirror coverage from marked-GFM behavior

Status update (2026-03-08, E22 compat harness correction and tilde delimiter cleanup complete):

- compat baseline moved from `33` to `21`
- completed in this batch:
  - `tests/compat_snapshot.rs` now treats `test/specs/gfm/commonmark.*.json` as CommonMark mirror coverage rather than forcing `gfm=true`; this removes a false-failure cluster from the compat harness instead of distorting parser behavior to satisfy the wrong mode
  - triple-tilde inline runs such as `~~~not~~~` no longer get split into nested `<del>` nodes; GFM `~` delimiters are now limited to run lengths that marked actually tokenizes for strikethrough
  - added direct regression coverage for the CommonMark mirror mode-selection rule and for GFM example `13`
- promoted regressions now enforced and passing:
  - `gfm example 13`
  - `compat_commonmark_mirror_cases_do_not_force_gfm`
- notable recovered cases:
  - `test/specs/gfm/commonmark.0.31.2.json#example-170`
  - `test/specs/gfm/commonmark.0.31.2.json#example-171`
  - `test/specs/gfm/commonmark.0.31.2.json#example-172`
  - `test/specs/gfm/commonmark.0.31.2.json#example-173`
  - `test/specs/gfm/commonmark.0.31.2.json#example-176`
  - `test/specs/gfm/commonmark.0.31.2.json#example-178`
  - `test/specs/gfm/commonmark.0.31.2.json#example-602`
  - `test/specs/gfm/commonmark.0.31.2.json#example-606`
  - `test/specs/gfm/commonmark.0.31.2.json#example-608`
  - `test/specs/gfm/commonmark.0.31.2.json#example-611`
  - `test/specs/gfm/commonmark.0.31.2.json#example-612`
  - `test/specs/gfm/gfm.0.29.json#example-13`
- planning notes after this batch:
  - `new/em_and_reflinks` was rechecked against `marked@17.0.4` runtime and the vendored `.html` fixture does not match current marked output; keep it deprioritized until fixture provenance is clarified
  - most of the remaining inline-tail cases are now pretty-HTML whitespace differences rather than parser-shape mismatches
  - the remaining high-value parser work is concentrated in block/container fixtures: `paragraph-after-list-item`, `tasklist_blocks`, `indented_details`, `fences_breaking_paragraphs`, and `toplevel_paragraphs`

Status update (2026-03-08, E23 tasklist first-line block-marker batch complete):

- compat baseline moved from `21` to `20`
- completed in this batch:
  - GFM task list items now force the first post-checkbox content line through paragraph parsing, so `# heading`, `> blockquote`, `---`, fenced-code openers, refdefs, raw HTML starts, and GFM tables stay literal inside the task item's first line the same way marked renders them
  - task list items no longer register reference definitions from their first line after stripping `[x]` / `[ ]`, which fixes empty-list-item regressions such as `- [x] [def]: ...`
  - parser regression normalization now canonicalizes `<input>` attribute order, matching the compat harness and removing noise from tasklist-focused regression gates
- promoted regressions now enforced and passing:
  - `new/tasklist_blocks.md`
- notable recovered cases:
  - `new/tasklist_blocks.md`
- remaining high-yield work after this batch:
  - block/container tails with real structure gaps: `paragraph-after-list-item`, `indented_details`, `fences_breaking_paragraphs`, `toplevel_paragraphs`
  - legacy Markdown paragraph/list behavior: `original/hard_wrapped_paragraphs_with_list_like_lines`, `original/tabs`
  - mostly formatting-leaning tails still left in compat: `blockquote_setext`, `em_list_links`, `emoji_strikethrough`, `emphasis_extra tests`, `unicode_punctuation`, `list_align_pedantic`, `list_item_text`

Status update (2026-03-08, E24 regression-normalization alignment and pedantic refdef guard complete):

- compat baseline moved from `20` to `19`
- completed in this batch:
  - reference-definition multiline continuation now stops before swallowing a following standalone definition line; focused block coverage was added for adjacent pedantic refdefs with indentation
  - `tests/parser_regressions.rs` now uses the same HTML normalization rules as `tests/compat_snapshot.rs`, so entity-spelling differences such as `&quot;` vs `"` no longer create false regression failures
  - `original/markdown_documentation_basics.md` was promoted into parser regressions and removed from the compat xfail baseline once the stronger normalization confirmed it is already matched
  - `new/list_align_pedantic.md` remains a known compat gap; its parser-regression test is now explicitly ignored instead of passing via weaker normalization
- promoted regressions now enforced and passing:
  - `original/markdown_documentation_basics.md`
  - focused pedantic refdef continuation coverage in `tests/parser_blocks.rs`
- notable recovered cases:
  - `original/markdown_documentation_basics.md`
- remaining high-yield work after this batch:
  - real compat deltas still left in the runtime-verified set are led by `new/em_list_links.md`
  - stale vendored fixtures still mixed into compat include `blockquote_setext`, `code_block_no_ending_newline`, `fences_breaking_paragraphs`, `indented_details`, `indented_tables`, `paragraph-after-list-item`, `toplevel_paragraphs`, and `original/tabs`
  - the next worthwhile parser/runtime target is the tight-list whitespace family around `em_list_links`; after that the remaining work is mostly either stale fixture drift or known pedantic whitespace tails

Status update (2026-03-08, E25 runtime-drift triage tooling added):

- compat baseline remains `19`
- completed in this batch:
  - added `scripts/check-marked-runtime-drift.mjs` and `npm run test:compat:runtime`
  - the new script installs the vendored `marked` version in a temp directory, re-renders compat fixtures with the same front-matter options as the Rust harness, and classifies each checked fixture as `MATCH` or `STALE` after using the same HTML normalization rules as the compat suite
  - current xfail scan result is `checked=19`, `match=1`, `stale=18`, which confirms that most remaining xfails are no longer good parser targets against current `marked@17.0.4`
- notable findings from this batch:
  - the only runtime-aligned xfail left is `new/em_list_links.md`
  - stale vendored xfails now explicitly confirmed by the script include `blockquote_setext`, `code_block_no_ending_newline`, `fences_breaking_paragraphs`, `indented_details`, `indented_tables`, `paragraph-after-list-item`, `toplevel_paragraphs`, `original/markdown_documentation_syntax`, and `original/tabs`
  - manual runtime probes also showed a tighter conflict in nested-list whitespace: current `marked@17.0.4` omits the paragraph-to-sublist newline for cases like `em_list_links`, `nested_blockquote_in_list`, and CommonMark examples `307` and `319`, while the vendored fixtures/spec snapshots still preserve that newline in some non-xfail cases
- planning notes after this batch:
  - do not keep pushing parser/renderer changes directly against the remaining vendored xfail list; the harness target has diverged too far from current upstream runtime
  - the next engineering step should be either:
    - split “vendored snapshot compatibility” from “current marked runtime compatibility”, or
    - refresh the vendored marked fixtures/spec snapshots before continuing parser work on the remaining list

Status update (2026-03-08, E26 compat harness split into snapshot gate and runtime audit):

- compat baseline remains `19`
- completed in this batch:
  - renamed the Rust integration harness from `tests/compat_marked.rs` to `tests/compat_snapshot.rs` so the gated fixture/spec check is explicitly a vendored snapshot target
  - `package.json` now exposes `npm run test:compat:snapshot`, `npm run test:compat:runtime`, and keeps `npm run test:compat` as an aggregate that runs both layers in sequence
  - snapshot baseline updates are now explicit through `npm run test:compat:snapshot:update-xfail`, and the `tests/compat/xfail.yaml` header now labels the file as snapshot-only
  - README and requirements docs now describe the split between vendored snapshot compatibility and current runtime drift auditing
- planning notes after this batch:
  - parser work can now target either the vendored snapshot gate or the current runtime audit without pretending they are the same compatibility objective
  - remaining entries in `tests/compat/xfail.yaml` should be treated as snapshot deltas unless they are separately revalidated against `npm run test:compat:runtime`

Status update (2026-03-08, E27 current-marked runtime gate added):

- snapshot baseline remains `19`
- runtime baseline initialized at `150`
- completed in this batch:
  - extracted shared compat helpers into `tests/compat_support/mod.rs` so snapshot/runtime suites use the same fixture collection, front-matter parsing, HTML normalization, and xfail YAML handling
  - added `tests/compat_runtime.rs`, which batches every compat fixture through a Node oracle backed by `marked@17.0.4` and compares the normalized runtime HTML against `markrs`
  - added `scripts/render-marked-runtime.mjs` as the batch renderer used by the Rust runtime suite
  - `package.json` now treats `npm run test:compat:runtime` as the real current-runtime gate, with `npm run test:compat:runtime:update-xfail` for its baseline and `npm run test:compat:runtime-drift` reserved for the older snapshot-drift audit
  - runtime baseline is stored in `tests/compat/runtime_xfail.yaml`
- notable findings from this batch:
  - the runtime mismatch surface is much larger than the remaining snapshot xfail list: `150` total runtime xfails versus `19` snapshot xfails
  - current runtime xfails break down as `59` CommonMark mirror cases, `65` GFM cases, `18` `new/` fixtures, and `8` `original/` fixtures
  - this confirms the current parser is closer to the vendored snapshots than to the current `marked` runtime on several clusters, so future compatibility work needs an explicit choice of target
- planning notes after this batch:
  - if the goal is upstream-current parity, the next reduction pass should come from `tests/compat/runtime_xfail.yaml`, not from `tests/compat/xfail.yaml`
  - `npm run test:compat:runtime-drift` remains useful to identify stale vendored snapshot entries, but it is no longer a substitute for measuring actual runtime compatibility

Status update (2026-03-08, E28 tight-list runtime whitespace batch complete):

- snapshot baseline moved from `19` to `36`
- runtime baseline moved from `150` to `132`
- completed in this batch:
  - tight list rendering no longer inserts an extra separator between the first paragraph payload and following block children, which matches current `marked@17.0.4` for nested sublists, headings, and several task-list block combinations
  - added direct renderer coverage for tight-list block joining and updated own rendering coverage to assert the no-separator heading form
  - snapshot-only regressions that now intentionally diverge from current runtime were marked `ignored` in `tests/parser_regressions.rs` instead of continuing to block runtime-oriented work under the old vendored target
  - snapshot/runtime baselines were both refreshed after the renderer change
- notable recovered runtime cases from this batch:
  - `new/em_list_links.md`
  - `new/list_align_pedantic.md`
  - `new/list_code_header.md`
  - `new/list_item_empty.md`
  - `new/nested_blockquote_in_list.md`
  - CommonMark mirror examples `9`, `294`, `296`, `307`, `319`, `323`
  - GFM mirror examples `9`, `294`, `296`, `307`, `319`, `323`
  - `test/specs/gfm/gfm.0.29.json#example-10`
- planning notes after this batch:
  - the next high-yield runtime clusters are now the indentation-preservation family (`example-2`, `113`, `133`, `222-226`, `241`, `new/toplevel_paragraphs`, `new/indented_tables`, `original/tabs`) and the attribute/autolink entity family (`original/amps_and_angles_encoding`, `original/auto_links`)
