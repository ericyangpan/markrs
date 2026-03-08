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

## Release

Push a semver tag like `v0.1.0`.

GitHub Actions workflow `.github/workflows/release.yml` will:

1. Build each platform binary.
2. Pack and publish platform npm packages.
3. Publish the main package `markrs`.
