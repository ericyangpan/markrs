# markrs

`markrs` is a Rust Markdown renderer distributed through npm.

By default it outputs HTML fragments like `marked`.
It can also output a full HTML document with built-in or custom styles.

## Install

```bash
npm i -g markrs
```

## Usage

Render Markdown to HTML fragment (default):

```bash
markrs README.md > out.html
cat README.md | markrs
```

Render full HTML document with built-in theme:

```bash
markrs --document --theme github README.md > page.html
markrs --document --theme dracula README.md > page.html
markrs --document --theme paper README.md > page.html
```

Apply custom style definition (JSON):

```bash
markrs --document --theme-file theme.json README.md > page.html
```

`theme.json` format:

```json
{
  "variables": {
    "--markrs-bg": "#0f1115",
    "--markrs-fg": "#f2f5f9",
    "--markrs-link": "#65c1ff"
  },
  "css": ".markrs h1 { letter-spacing: 0.02em; }"
}
```

Append extra CSS file:

```bash
markrs --document --css ./extra.css README.md > page.html
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
Current default and only parser is the in-house `markdown` module (new parser pipeline), with no external markdown engine dependency.

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
- `third_party/marked/test/unit/*.test.js`: 158 JS unit cases. These exercise Marked's JS API surface such as hooks, lexer/parser classes, CLI integration, and instance behavior, so there is no 1:1 Rust-side case mapping in `markrs` yet.
- `third_party/marked/test/specs/redos`: 7 ReDoS fixtures. These are security/performance-oriented fixtures and are not currently part of the `markrs` compat gates.

| Target | Case source | Passed | Gaps | Pass rate |
| --- | --- | ---: | ---: | ---: |
| `marked` self-spec result | vendored `marked` fixture/spec corpus | 1485 | 0 | 100.0% |
| `markrs` snapshot compat | vendored fixture/spec snapshots | 1449 | 36 | 97.6% |
| `markrs` runtime compat | current `marked@17.0.4` runtime | 1353 | 132 | 91.1% |

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
- `markrs` through an in-process Rust benchmark binary
- `pulldown-cmark` through an in-process Rust benchmark binary
- `marked` through `marked.parse(...)`
- `markdown-it` through `markdown-it.render(...)`
- `remark` through `remark + remark-gfm + remark-html`

`CommonMark Core` is the fairest suite for `pulldown-cmark`, because it runs the official CommonMark examples with `gfm=false`.

Raw data is written to `bench/results/latest.json`.

Performance strategy and optimization batches live in `docs/performance.md`.

`pulldown-cmark` is included as a throughput ceiling reference. `markrs` is not expected to match its architecture or semantics in Phase 1.

<!-- benchmark-report:start -->
Benchmark date: 2026-03-09

Method: in-process render throughput on the same default-GFM corpus for all engines. Outputs are not normalized for semantic equality; this report only measures rendering speed on shared inputs.

Environment: Apple M4 | darwin 24.6.0 (arm64) | Node 22.12.0 | Rust rustc 1.93.0 (254b59607 2026-01-19)

| Suite | Docs | Input size | Warmup | Measured | Source |
| --- | ---: | ---: | ---: | ---: | --- |
| README.md | 1 | 7.1 KiB | 10 | 30 | Project README rendered as a single document |
| CommonMark Core | 652 | 14.6 KiB | 4 | 10 | Official CommonMark 0.31.2 JSON examples rendered in non-GFM mode |
| Marked Fixtures | 153 | 58.3 KiB | 4 | 12 | `new` + `original` fixture pairs from vendored marked specs |
| Comparable Corpus | 1485 | 88.9 KiB | 2 | 6 | All 1485 comparable parser-output cases from vendored marked specs |

| Suite | Engine | Mean ms | Median ms | Docs/s | MiB/s | vs marked |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| README.md | markrs (Rust) | 0.20 | 0.19 | 4900.4 | 34.01 | 3.21x |
| README.md | pulldown-cmark (Rust) | 0.03 | 0.02 | 37117.3 | 257.63 | 24.34x |
| README.md | marked (JS) | 0.66 | 0.48 | 1524.8 | 10.58 | 1.00x |
| README.md | markdown-it (JS) | 0.44 | 0.38 | 2290.4 | 15.90 | 1.50x |
| README.md | remark + gfm + html | 7.05 | 6.15 | 141.9 | 0.98 | 0.09x |
| CommonMark Core | markrs (Rust) | 0.89 | 0.85 | 729309.8 | 15.91 | 2.46x |
| CommonMark Core | pulldown-cmark (Rust) | 0.28 | 0.27 | 2334268.7 | 50.94 | 7.88x |
| CommonMark Core | marked (JS) | 2.20 | 1.97 | 296130.9 | 6.46 | 1.00x |
| CommonMark Core | markdown-it (JS) | 2.24 | 2.05 | 290898.8 | 6.35 | 0.98x |
| CommonMark Core | remark + gfm + html | 33.35 | 32.40 | 19548.9 | 0.43 | 0.07x |
| Marked Fixtures | markrs (Rust) | 3.71 | 3.61 | 41233.4 | 15.34 | 3.12x |
| Marked Fixtures | pulldown-cmark (Rust) | 0.34 | 0.32 | 456621.7 | 169.86 | 34.59x |
| Marked Fixtures | marked (JS) | 11.59 | 8.98 | 13201.1 | 4.91 | 1.00x |
| Marked Fixtures | markdown-it (JS) | 3.15 | 3.07 | 48529.1 | 18.05 | 3.68x |
| Marked Fixtures | remark + gfm + html | 76.91 | 70.08 | 1989.3 | 0.74 | 0.15x |
| Comparable Corpus | markrs (Rust) | 5.62 | 5.59 | 264411.3 | 15.46 | 1.95x |
| Comparable Corpus | pulldown-cmark (Rust) | 1.02 | 0.98 | 1462894.0 | 85.52 | 10.79x |
| Comparable Corpus | marked (JS) | 10.95 | 10.51 | 135603.3 | 7.93 | 1.00x |
| Comparable Corpus | markdown-it (JS) | 6.15 | 6.15 | 241481.9 | 14.12 | 1.78x |
| Comparable Corpus | remark + gfm + html | 200.40 | 194.11 | 7410.1 | 0.43 | 0.05x |

Raw benchmark data: `bench/results/latest.json`
<!-- benchmark-report:end -->

## Release

Push a semver tag like `v0.1.0`.

GitHub Actions workflow `.github/workflows/release.yml` will:

1. Build each platform binary.
2. Pack and publish platform npm packages.
3. Publish the main package `markrs`.
