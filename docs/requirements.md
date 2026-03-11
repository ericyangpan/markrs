# markrs Requirements

Last updated: 2026-03-12

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
- Isolated benchmark runs keep `Comparable Corpus` at `1.25x` or better vs `marked`, with `Marked Fixtures` at or above parity.

Default aggressive execution target:

- Drive current-runtime compatibility to parity by reducing `tests/compat/runtime_xfail.yaml` from the current `83` gaps to `0`.
- Keep `tests/compat/xfail.yaml` on a downward trend from the current `81` gaps, but do not trade away runtime parity just to satisfy stale vendored snapshots.
- Preserve benchmark guardrails while reducing compatibility gaps:
  - `Comparable Corpus >= 1.25x` vs `marked`
  - `Marked Fixtures >= 1.00x` vs `marked`

Autonomous mode:

- Agents may continue through repeated parser/renderer reduction batches without waiting for user confirmation after each batch.
- Each batch is expected to include the code change, focused regression coverage, validation, any intentional xfail delta, and a commit.
- Stop and escalate only when:
  - a fix requires changing the public API, CLI behavior, packaging contract, or documented default semantics
  - a fix requires editing `third_party/marked/*` or changing the vendored/current `marked` target version
  - runtime parity and snapshot parity require conflicting behavior that the current harness cannot represent cleanly
  - three consecutive well-scoped batches fail to reduce the runtime gap
  - a correctness fix would push isolated benchmark results below the guardrail floor

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
