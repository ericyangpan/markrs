# Rename To markast

This document records the current state of the `markrs` -> `markast` rename and the remaining release work.

Status date: 2026-03-15

## Completed

- Repository renamed to `ericyangpan/markast`
- Cargo package renamed from `markrs` to `markast`
- CLI command renamed from `markrs` to `markast`
- npm root package renamed from `markrs` to `markast`
- npm platform package names renamed from `markrs-*` to `markast-*`
- Public CSS contract renamed from `--markrs-*` / `.markrs` to `--markast-*` / `.markast`
- README, docs, tests, scripts, and release workflow updated to the new name
- `markast v0.1.0` published to `crates.io`
- `markrs v0.1.0` yanked from `crates.io`
- `markast@0.1.0` published to npm
- Published npm platform packages:
  - `markast-darwin-arm64@0.1.0`
  - `markast-darwin-x64@0.1.0`
  - `markast-linux-arm64-gnu@0.1.0`
  - `markast-linux-x64-gnu@0.1.0`
- Deprecated legacy npm platform packages:
  - `markrs-darwin-arm64`
  - `markrs-darwin-x64`
  - `markrs-linux-arm64-gnu`
  - `markrs-linux-x64-gnu`

## Remaining Issue

The only incomplete external artifact is:

- `markast-win32-x64-msvc@0.1.0`

Local Windows binary generation was completed with a GNU Windows target to prove the release shape, but npm rejected publishing the `markast-win32-x64-msvc` package name with:

`403 Forbidden - Package name triggered spam detection`

This is an npm registry policy block, not a local build failure and not an authentication problem.

## Current External State

- GitHub repo: `https://github.com/ericyangpan/markast`
- crates.io crate: `markast`
- npm root package: `markast`
- Missing npm package: `markast-win32-x64-msvc`

## Recommended Next Step

File an npm support request for the blocked Windows package name and include:

- package name: `markast-win32-x64-msvc`
- owner account: `arielyang`
- publish attempt date: 2026-03-15
- error: `403 Forbidden - Package name triggered spam detection`
- root package already published: `markast@0.1.0`
- this package is the Windows companion package referenced by the published root package as an `optionalDependency`
- note that related packages under the same namespace were successfully published:
  - `markast`
  - `markast-darwin-arm64`
  - `markast-darwin-x64`
  - `markast-linux-arm64-gnu`
  - `markast-linux-x64-gnu`

## npm Support Draft

Suggested subject:

`Spam-detection block on publish for markast-win32-x64-msvc`

Suggested message:

```text
Hello npm Support,

I am the maintainer of the `markast` package set.

I successfully published the main package and four companion platform packages for the same release:

  markast@0.1.0
  markast-darwin-arm64@0.1.0
  markast-darwin-x64@0.1.0
  markast-linux-arm64-gnu@0.1.0
  markast-linux-x64-gnu@0.1.0

The only remaining companion package is the Windows package:

  markast-win32-x64-msvc@0.1.0

When I try to publish it, npm rejects the request with:

  403 Forbidden - Package name triggered spam detection

This does not appear to be an authentication or local packaging problem.

Project details:

  npm owner account: arielyang
  GitHub repository: https://github.com/ericyangpan/markast
  publish attempt date: 2026-03-15

This package is the Windows prebuilt binary companion for the same project and release line as the packages above.
It is also referenced by the already-published `markast@0.1.0` package as an optional dependency for Windows users.

Could you please review the spam-detection block on `markast-win32-x64-msvc` and either unblock the package name or let me know the required next step to get it approved?

If you need any additional verification or package metadata from me, I can provide it.

Thank you.
```

Recommended attachments or extra details for the support ticket:

- pasted `npm publish` command and terminal output showing the `403 Forbidden - Package name triggered spam detection` error
- link to the published `markast@0.1.0` package
- link or snippet showing `markast-win32-x64-msvc@0.1.0` listed in `markast` `optionalDependencies`
- confirmation that the Windows package contents match the already-published darwin/linux companion package pattern

## Validation Already Run

- `cargo test --all-targets`
- `npm run check:npm-versions`
- `npm run check:strict`
- `cargo package --allow-dirty`
- `npm pack --dry-run`
- `target/release/markast --help`
