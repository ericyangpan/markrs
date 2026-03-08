# markrs Requirements

Last updated: 2026-03-05

## Product Direction

`markrs` is an HTML-output Markdown renderer that targets compatibility with `marked` while adding project-specific styling features.

## Priority Roadmap

### P0 (Primary Mission)

- Replace `pulldown-cmark` with an in-house parser implementation.
- Build the parser from scratch in Rust and make it the default parsing core.
- Keep passing:
  - markrs own test suite
  - marked snapshot compatibility suite (with shrinking `tests/compat/xfail.yaml`)
  - current marked runtime compatibility suite (with shrinking `tests/compat/runtime_xfail.yaml`)

Acceptance criteria:

- `Cargo.toml` no longer depends on `pulldown-cmark`.
- The new Rust parser is used by `render_markdown_to_html`.
- `npm run test:own` and `npm run test:compat` pass in CI.
- `npm run test:compat` aggregates both `test:compat:snapshot` and `test:compat:runtime`.

Implementation plan:

- `docs/p0-parser-plan.md`

### P1 (Security Mode)

- Add optional HTML sanitization support for output.
- Preserve marked-compat mode by keeping sanitize disabled by default.

Acceptance criteria:

- Provide explicit sanitize toggle in API/CLI.
- Add dedicated sanitize tests separate from compat tests.

### P2 (WASM Runtime Support)

- Add WASM build output for browser/edge/runtime fallback.
- Keep native prebuilt binaries as the primary Node path.

Acceptance criteria:

- Shared Rust core for native and WASM outputs.
- NPM package supports native-first loading with WASM fallback.
