# markast

`markast` is a Rust Markdown renderer distributed through npm.

By default it outputs HTML fragments like `marked`.
It can also output a full HTML document with built-in or custom styles.

## Install

```bash
npm i -g markast
```

## Usage

Render Markdown to HTML fragment (default):

```bash
markast README.md > out.html
cat README.md | markast
```

Render full HTML document with built-in theme:

```bash
markast --document --theme github README.md > page.html
markast --document --theme dracula README.md > page.html
markast --document --theme paper README.md > page.html
```

Apply custom style definition (JSON):

```bash
markast --document --theme-file theme.json README.md > page.html
```

`theme.json` format:

```json
{
  "variables": {
    "--markast-bg": "#0f1115",
    "--markast-fg": "#f2f5f9",
    "--markast-link": "#65c1ff"
  },
  "css": ".markast h1 { letter-spacing: 0.02em; }"
}
```

Append extra CSS file:

```bash
markast --document --css ./extra.css README.md > page.html
```

## Development

```bash
npm run check
npm run check:strict
npm run test:own
npm run test:compat:snapshot
npm run test:compat:runtime
npm run test:compat
npm run test:compat:report
npm run build
```

Parser engine:
Current default and only production parser is the in-house `markdown` module (new parser pipeline), with no external markdown engine dependency in the main crate.

Requirements and roadmap: `docs/requirements.md`

Compatibility fixtures are synced under `third_party/marked/test/specs`.

Compat now has two layers:

- `npm run check:strict`: runs Rust compile/test gates with warnings denied.
- `npm run test:compat:snapshot`: gated comparison against vendored marked fixture/spec snapshots.
- `npm run test:compat:runtime`: gated comparison against the current vendored `marked` npm runtime.
- `npm run test:compat:runtime-drift`: auxiliary audit that checks whether snapshot-xfailed vendored fixtures still match the current runtime.
- `npm run test:compat`: runs both in sequence.

Known snapshot gaps are tracked in `tests/compat/xfail.yaml`.
Known runtime gaps are tracked in `tests/compat/runtime_xfail.yaml`.

Refresh the snapshot xfail baseline after intentional parser behavior changes:

```bash
npm run test:compat:snapshot:update-xfail
```

Refresh the runtime xfail baseline after intentional parser behavior changes:

```bash
npm run test:compat:runtime:update-xfail
```

## Compatibility Report

Current report date: 2026-03-08

This table compares the same parser-output cases from the official marked corpus under `third_party/marked/test/specs`.

Included in the same-case comparison:
- `new` + `original` fixture pairs: 153
- CommonMark JSON examples: 652
- GFM CommonMark mirror examples: 652
- GFM spec examples: 28
- Total comparable cases: 1485

Excluded from this table:
- `third_party/marked/test/unit/*.test.js`: 158 JS unit cases. These exercise Marked's JS API surface such as hooks, lexer/parser classes, CLI integration, and instance behavior, so there is no 1:1 Rust-side case mapping in `markast` yet.
- `third_party/marked/test/specs/redos`: 7 ReDoS fixtures. These are security/performance-oriented fixtures and are not currently part of the `markast` compat gates.

| Target | Case source | Passed | Gaps | Pass rate |
| --- | --- | ---: | ---: | ---: |
| `marked` self-spec result | vendored `marked` fixture/spec corpus | 1485 | 0 | 100.0% |
| `markast` snapshot compat | vendored fixture/spec snapshots | 1449 | 36 | 97.6% |
| `markast` runtime compat | current `marked@17.0.4` runtime | 1353 | 132 | 91.1% |

How to refresh:
- `npm run test:compat`
- `npm run test:compat:report`

## Benchmark

Reproduce locally:

```bash
npm install
npm run bench
```

The harness benchmarks shared Markdown corpora against five engines:
- `markast` through an in-process Rust benchmark binary
- `pulldown-cmark` through a benchmark-only comparison runner
- `marked` through `marked.parse(...)`
- `markdown-it` through `markdown-it.render(...)`
- `remark` through `remark + remark-gfm + remark-html`

`CommonMark Core` is the fairest suite for `pulldown-cmark`, because it runs the official CommonMark examples with `gfm=false`.

Raw data is written to `bench/results/latest.json`.

Performance strategy and optimization batches live in `docs/performance.md`.

`pulldown-cmark` is included as a throughput ceiling reference. `markast` is not expected to match its architecture or semantics in Phase 1.
The `pulldown-cmark` comparator is kept outside the main `markast` crate so release/runtime dependencies stay focused on the in-house parser.

<!-- benchmark-report:start -->
Benchmark date: 2026-03-13

Method: in-process render throughput on the same default-GFM corpus for all engines. Outputs are not normalized for semantic equality; this report only measures rendering speed on shared inputs. `Trimmed mean ms` drops one run from each side for 6-9 samples, or 10% from each side for 10+ samples.

Environment: Apple M4 | darwin 24.6.0 (arm64) | Node 22.22.1 | Rust rustc 1.93.0 (254b59607 2026-01-19)

| Suite | Docs | Input size | Warmup | Measured | Source |
| --- | ---: | ---: | ---: | ---: | --- |
| README.md | 1 | 7.2 KiB | 10 | 30 | Project README rendered as a single document |
| CommonMark Core | 652 | 14.6 KiB | 4 | 10 | Official CommonMark 0.31.2 JSON examples rendered in non-GFM mode |
| Marked Fixtures | 153 | 58.3 KiB | 4 | 12 | `new` + `original` fixture pairs from vendored marked specs |
| Comparable Corpus | 1485 | 88.9 KiB | 4 | 12 | All 1485 comparable parser-output cases from vendored marked specs |

| Suite | Engine | Trimmed mean ms | Median ms | Docs/s | MiB/s | vs marked |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| README.md | markast (Rust) | 0.24 | 0.24 | 4116.9 | 29.02 | 1.64x |
| README.md | pulldown-cmark (Rust) | 0.05 | 0.05 | 21819.0 | 153.79 | 8.71x |
| README.md | marked (JS) | 0.40 | 0.39 | 2506.1 | 17.66 | 1.00x |
| README.md | markdown-it (JS) | 0.47 | 0.46 | 2133.9 | 15.04 | 0.85x |
| README.md | remark + gfm + html | 5.40 | 5.20 | 185.0 | 1.30 | 0.07x |
| CommonMark Core | markast (Rust) | 1.23 | 1.20 | 529408.8 | 11.55 | 1.62x |
| CommonMark Core | pulldown-cmark (Rust) | 0.52 | 0.51 | 1258143.2 | 27.46 | 3.85x |
| CommonMark Core | marked (JS) | 1.99 | 1.93 | 327005.7 | 7.14 | 1.00x |
| CommonMark Core | markdown-it (JS) | 2.33 | 2.26 | 280410.0 | 6.12 | 0.86x |
| CommonMark Core | remark + gfm + html | 29.32 | 28.71 | 22234.4 | 0.49 | 0.07x |
| Marked Fixtures | markast (Rust) | 3.24 | 3.17 | 47214.0 | 17.56 | 1.41x |
| Marked Fixtures | pulldown-cmark (Rust) | 0.59 | 0.59 | 259931.5 | 96.69 | 7.77x |
| Marked Fixtures | marked (JS) | 4.57 | 4.57 | 33466.2 | 12.45 | 1.00x |
| Marked Fixtures | markdown-it (JS) | 3.69 | 3.64 | 41492.2 | 15.43 | 1.24x |
| Marked Fixtures | remark + gfm + html | 47.08 | 46.84 | 3250.1 | 1.21 | 0.10x |
| Comparable Corpus | markast (Rust) | 4.41 | 4.41 | 336770.0 | 19.69 | 1.90x |
| Comparable Corpus | pulldown-cmark (Rust) | 1.23 | 1.20 | 1206282.9 | 70.52 | 6.80x |
| Comparable Corpus | marked (JS) | 8.37 | 8.37 | 177508.4 | 10.38 | 1.00x |
| Comparable Corpus | markdown-it (JS) | 6.94 | 6.90 | 213845.0 | 12.50 | 1.20x |
| Comparable Corpus | remark + gfm + html | 140.03 | 140.38 | 10604.6 | 0.62 | 0.06x |

Raw benchmark data: `bench/results/latest.json`
<!-- benchmark-report:end -->

## Release

Push a semver tag like `v0.1.0`.

GitHub Actions workflow `.github/workflows/release.yml` will:

1. Build each platform binary.
2. Pack and publish platform npm packages.
3. Publish the main package `markast`.
