# Performance

This document turns benchmark observations into a concrete optimization plan for `markast`.

It has two jobs:

- define the benchmark target we are actually optimizing for
- explain which implementation ideas are worth borrowing from `pulldown-cmark` and `micromark`

## Goal

Phase 1 target:

- beat `marked` on `CommonMark Core`
- beat `marked` on `Marked Fixtures`
- beat `marked` on `Comparable Corpus`
- hold the lead across repeated local benchmark runs, not just a single best run

Phase 2 target:

- reach `1.25x` or better vs `marked` on `Comparable Corpus`
- keep `Marked Fixtures` at or above parity

Non-goals for now:

- matching `pulldown-cmark` throughput
- sacrificing compatibility to chase a benchmark number
- changing benchmark inputs to manufacture a win

## Benchmark Contract

Run:

```bash
npm install
npm run bench
```

Implementation note:

- the `pulldown-cmark` comparison runner lives under `bench/pulldown-cmark-runner`, not in the main `markast` crate dependency graph

Measurement discipline:

- run benchmarks in isolation; do not overlap them with `npm run check:strict`, `cargo test`, or other heavy local workloads

Current benchmark suites:

- `README.md`
- `CommonMark Core`
- `Marked Fixtures`
- `Comparable Corpus`

Interpretation:

- `CommonMark Core` is the fairest suite for `pulldown-cmark`
- `Marked Fixtures` and `Comparable Corpus` are the fairest suites for `marked`, `markdown-it`, and `markast`
- `README.md` is useful for spotting fixed overhead, but it is not the primary KPI

Primary KPI:

- `Comparable Corpus` trimmed mean and median vs `marked`

Secondary KPI:

- `Marked Fixtures` trimmed mean and median vs `marked`

Guardrails:

- `npm run check:strict`
- `npm run test:compat`
- benchmark report updated only after the improvement is real

## What `pulldown-cmark` Proves

`pulldown-cmark` is much faster because its architecture is optimized for throughput:

- pull-parser design instead of AST-first construction
- minimal allocation and copying
- heavy use of borrowed input slices
- low-level scanning over byte patterns

What to borrow:

- avoid building temporary owned strings on the hot path
- prefer borrowed slices or lightweight views over reconstructed text
- treat AST construction as a cost center, not a default right

What not to copy blindly:

- `markast` still has to preserve `marked` compatibility behavior that `pulldown-cmark` does not target
- full event-stream architecture is a larger rewrite than we need for Phase 1

## What `micromark` Proves

`micromark` is useful as an architecture reference even though it is not in the main benchmark table.

Ideas worth borrowing:

1. State-machine parsing
- syntax is resolved through explicit scanning states instead of repeated ad hoc rescans

2. Chunk-oriented processing
- do not eagerly materialize the whole inline input as `Vec<char>`
- keep work close to byte slices and offsets

3. Pipeline boundaries
- `preprocess -> parse -> postprocess -> compile`
- parsing and HTML generation should be separate concerns

4. Extension boundaries
- syntax hooks and HTML hooks should not be mixed into one giant parser branch table

For `markast`, the practical takeaway is simple:

- less `String`
- less `Vec<char>`
- less rescanning
- clearer boundaries between syntax recognition and rendering

## Current Cost Centers

The current parser is correct enough to benchmark, but still pays visible overhead in a few places:

1. Inline parsing
- `src/markdown/inline.rs`
- still leans on whole-input character materialization and multiple passes through the same text

2. Delimiter resolution
- `src/markdown/inline.rs`
- emphasis and link/image resolution still clone or rebuild intermediate structures more than they should

3. Block and list reconstruction
- `src/markdown/block.rs`
- some paths still rebuild text that was already available as slices

4. Source normalization and line scanning
- `src/markdown/source.rs`
- `src/markdown/lexer.rs`
- `src/markdown/parser.rs`
- the cheap paths are better than before, but there is still room to reduce fixed overhead

5. Renderer allocations
- `src/markdown/render.rs`
- escaping and string assembly can still avoid some repeated growth and temporary strings

## Optimization Program

### Batch A: Inline Scanner Rewrite

Target files:

- `src/markdown/inline.rs`

Goal:

- replace whole-input `Vec<char>` usage on the top inline path with byte-offset or chunk-based scanning

Work items:

- keep original input as `&str`
- scan with offsets or `char_indices()`
- only materialize substrings when an inline node actually needs owned text
- preserve the existing focused inline tests while changing the scanner internals

Expected payoff:

- lower fixed overhead on small docs
- better throughput on `Marked Fixtures`
- lower allocation count across all suites

Acceptance:

- no compat regression
- `Marked Fixtures` and `Comparable Corpus` both improve vs current baseline

### Batch B: Delimiter and Link Resolution Cleanup

Target files:

- `src/markdown/inline.rs`
- `src/markdown/block.rs`

Goal:

- reduce rescanning and temporary structure rebuilding during emphasis and reference/link resolution

Work items:

- collapse duplicated label validation and normalization passes
- avoid cloning delimiter-side buffers when a borrowed view is enough
- keep plain-text fast paths hot when no special syntax is present

Expected payoff:

- better small-fixture performance
- less jitter around parity with `marked`

Acceptance:

- `Marked Fixtures` median stays above `marked`

### Batch C: Container and List Slice Preservation

Target files:

- `src/markdown/block.rs`

Goal:

- stop rebuilding block/list text through `join(...)` and re-splitting where the original slices already exist

Work items:

- preserve line slice ranges deeper into list parsing
- pass slice windows instead of reconstructed strings
- keep continuation and indentation logic source-based

Expected payoff:

- lower medium-corpus cost
- better `Comparable Corpus` throughput

Acceptance:

- `Comparable Corpus` mean improves again after Batch B

### Batch D: Fixed Overhead Reduction

Target files:

- `src/markdown/source.rs`
- `src/markdown/lexer.rs`
- `src/markdown/parser.rs`

Goal:

- reduce work for small and single-document inputs

Work items:

- keep CRLF normalization lazy
- preserve single-line fast paths
- avoid creating scanner state that the input does not need

Expected payoff:

- better `README.md`
- better `Marked Fixtures`

### Batch E: Renderer Hot Path Cleanup

Target files:

- `src/markdown/render.rs`

Goal:

- reduce avoidable HTML assembly overhead without changing semantics

Work items:

- reserve output capacity more aggressively
- avoid temporary escaped strings when direct append is possible
- keep text-heavy paragraph rendering on a fast path

Expected payoff:

- modest gains across all suites
- especially useful after parser-side allocation work is done

## Architectural Direction After Phase 1

If `markast` beats `marked` but still sits far from `pulldown-cmark`, the next step is not random micro-optimization.

The next serious options are:

1. event-oriented internal representation for some block/inline paths
2. direct-render fast paths for simple documents
3. syntax-stage vs render-stage extension boundaries inspired by `micromark`

Those are Phase 2 topics, not prerequisites for beating `marked`.

## Working Rules

Each optimization batch should do all of the following:

1. change one cost center at a time
2. keep focused tests near the touched layer
3. run:

```bash
npm run check:strict
npm run test:compat
npm run bench
```

4. update `bench/results/latest.json`
5. update the README benchmark block only after the improvement is repeatable

## Current Interpretation

Right now the benchmark means:

- `markast` is already competitive with `marked`
- `markdown-it` is a stronger JS baseline than expected and should stay in the table
- `pulldown-cmark` is the throughput ceiling reference, not the immediate product target

That changes the engineering priority:

- first beat `marked` reliably
- then widen the gap
- only then decide how much `pulldown-cmark`-style architecture to adopt
