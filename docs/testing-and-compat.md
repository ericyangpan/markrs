# Testing And Compatibility

This document explains the repo's stable quality gates and compatibility baselines.

## Test Layers

`npm run check`

- compile-oriented sanity check

`npm test`

- runs `cargo test --all-targets`

`npm run check:strict`

- runs compile and test gates with Rust warnings denied
- this is the main CI safety net

`npm run test:own`

- project-owned behavior tests
- use this for CLI/document/theme expectations and stable product behavior

`cargo test --test parser_blocks`

- focused block parsing regressions

`cargo test --test parser_inlines`

- focused inline parsing regressions

`cargo test --test parser_regressions`

- targeted fixture-backed regressions

## Compatibility Gates

`npm run test:compat:snapshot`

- compares `markast` output to vendored `marked` fixture snapshots under `third_party/marked/test/specs`

`npm run test:compat:runtime`

- compares `markast` output to the current vendored `marked` npm runtime

`npm run test:compat`

- runs snapshot and runtime gates in sequence

`npm run test:compat:report`

- prints the current compatibility summary used by the project README

## Snapshot vs Runtime

Snapshot compatibility:

- stable against the vendored fixture corpus
- good for catching unintended output drift

Runtime compatibility:

- asks the currently vendored `marked` package to render the same inputs
- good for detecting drift between old fixture expectations and current `marked` behavior

Both matter because a parser change can satisfy one and regress the other.

## Xfail Baselines

Known gaps are tracked in:

- `tests/compat/xfail.yaml`
- `tests/compat/runtime_xfail.yaml`

These files are baselines, not a substitute for understanding failures.

Only update them when:

- the behavior change is intentional
- the new result has been inspected
- the baseline change is part of the same change set and explained in review

Commands:

```bash
npm run test:compat:snapshot:update-xfail
npm run test:compat:runtime:update-xfail
```

Helpful drift audit:

```bash
npm run test:compat:runtime-drift
```

## Fixture Sources

Primary fixture source:

- `third_party/marked/test/specs`

Support code:

- `tests/compat_support/mod.rs`
- `scripts/render-marked-runtime.mjs`
- `scripts/check-marked-runtime-drift.mjs`

Fixture sync helper:

```bash
scripts/sync-marked-specs.sh
```

Do not run fixture sync casually. It can change a large compatibility surface area.

## Suggested Validation By Change Type

Parser bug fix:

- run the most relevant focused parser test
- run `npm run test:own`
- run `npm run test:compat`

Renderer or autolink change:

- run `cargo test --test own_rendering`
- run `cargo test --test parser_regressions`
- run `npm run test:compat`

Packaging or workflow change:

- run `npm run check:npm-versions`
- run `npm run check:strict`

## Release Gate Checklist

Before release or merge of parser-affecting work:

1. `npm run check:strict`
2. `npm run test:own`
3. `npm run test:compat`
4. `npm run check:npm-versions` if package metadata changed
5. `npm run test:compat:report` if compatibility numbers in docs are being refreshed

## Benchmark Notes

Benchmarking is not part of the release gate, but it is useful after parser work:

```bash
npm run bench
```

Outputs:

- raw benchmark JSON in `bench/results/latest.json`
- optional README benchmark block refresh when invoked through the existing script path
- optimization strategy and benchmark interpretation in `docs/performance.md`

## Scope Note

This file documents stable gates and baselines.

It intentionally does not try to preserve temporary implementation procedures. Those belong in Git history, issues, or PR discussion unless they change the long-term working agreement.
