# markast Development Docs

This directory is the stable entrypoint for people and agents working on `markast`.

## Start Here

Read in this order when you are new to the repo:

1. `docs/README.md`
2. `docs/architecture.md`
3. `docs/testing-and-compat.md`
4. `docs/performance.md`

Read these when you are working on parser roadmap items:

- `docs/requirements.md`
- `docs/p0-parser-plan.md`
- `docs/p0-parser-design.md`

Read this when you need the rename and publish status:

- `docs/rename-to-markast.md`

## Project Snapshot

`markast` is a Rust Markdown renderer shipped through npm.

Core characteristics:

- CLI entrypoint in `src/main.rs`
- Public library API in `src/lib.rs`
- In-house Markdown parser under `src/markdown/*`
- npm wrapper package in the repo root and platform packages under `npm/*`
- Compatibility fixtures vendored from `marked` under `third_party/marked`

## Fast Path

For humans:

- Install Rust stable and Node.js 18+.
- Run `npm install`.
- Run `npm run check:strict`.
- Run `npm run test:compat` before merging parser-affecting work.

For agents:

- Check `git status --short` before editing.
- Assume the source of truth is code plus tests, not older roadmap notes.
- Prefer the narrowest validation that proves the change, then run broader gates if parser behavior changed.
- For P0 parser/runtime compatibility work, follow the autonomous execution loop in `docs/p0-parser-plan.md` and keep iterating until one of its escalation conditions triggers.
- Do not update `tests/compat/xfail.yaml` or `tests/compat/runtime_xfail.yaml` unless the behavior change is intentional and verified.
- Do not edit `third_party/marked/*` unless the task is explicitly about fixture sync.

## Doc Map

`docs/architecture.md`

- Rendering pipeline
- Module boundaries
- Where CLI, parser, renderer, and npm packaging connect

`docs/testing-and-compat.md`

- Test layers and when to run them
- Snapshot vs runtime compatibility
- Xfail baseline workflow
- Release gate checklist

`docs/performance.md`

- Benchmark contract and interpretation
- Performance targets vs `marked`
- Optimization program and hot-path priorities

`docs/rename-to-markast.md`

- Current rename completion status
- Published package inventory
- Remaining npm Windows package blocker

`docs/requirements.md`, `docs/p0-parser-plan.md`, `docs/p0-parser-design.md`

- Project direction
- Autonomous execution target and escalation conditions
- Parser roadmap and detailed design

## Documentation Scope

This directory prefers durable development facts over change history.

Good doc topics:

- code layout
- API and behavior contracts
- test and release gates
- roadmap and design intent

Usually not worth documenting here:

- temporary implementation steps
- one-off execution notes
- change logs that already exist in Git history

## Operating Principles

These rules make the repo easier for both humans and agents to change safely:

- Keep behavior changes covered by focused tests near the affected layer.
- Treat compatibility failures as product signals, not just test noise.
- Prefer small, source-located fixes over large speculative rewrites.
- Keep docs updated when the working agreement changes.
