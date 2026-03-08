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
npm run test:own
npm run test:compat:snapshot
npm run test:compat:runtime
npm run test:compat
npm run build
```

Parser engine:
Current default and only parser is the in-house `markdown` module (new parser pipeline), with no external markdown engine dependency.

Requirements and roadmap: `docs/requirements.md`

Compatibility fixtures are synced under `third_party/marked/test/specs`.

Compat now has two layers:

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

## Release

Push a semver tag like `v0.1.0`.

GitHub Actions workflow `.github/workflows/release.yml` will:

1. Build each platform binary.
2. Pack and publish platform npm packages.
3. Publish the main package `markrs`.
