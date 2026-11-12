# Contributing to Clash Verge App Proxy

This repository maintains the application-routing and Freecat derivative on `main`. Use [this repository](https://github.com/Avalon0409/clash-verge-rev) when developing or reporting changes for the custom client.

## Contribution Expectations

Describe the problem before implementing a non-trivial change and link its existing issue. If Issues are disabled, obtain a maintainer-approved problem report first. Keep each changed area tied to the reported behavior, preserve existing configuration and credential storage, and verify the result against the reproduction.

Repository and directory contracts live in [AGENTS.md](./AGENTS.md), [src/AGENTS.md](./src/AGENTS.md) and [src-tauri/AGENTS.md](./src-tauri/AGENTS.md). Add a regression test when it reproduces the reported bug or protects behavior that would otherwise go unchecked. Explain its purpose in the PR.

PRs from contributors without write access are checked by the ownership review and may receive `ai-slop:med` or `ai-slop:high` labels. Do not pad changes with speculative tests or defensive code; keep validation tied to the reported behavior.

## Branches

`main` owns the custom client and its releases. Start feature and fix branches from `main` and target it in pull requests. Agent-created working branches use the `codex/` prefix. The old `feat/app-proxy-freecat-v2.5.8` branch retains the development history that established this version.

The original project uses `dev` as its default development branch and `main` for stable releases. This fork keeps the lines separate:

| Reference | Purpose |
| --- | --- |
| `origin/main` | This custom client's main branch. |
| `upstream/main` | The original project's stable release line; the usual source for upstream updates. |
| `upstream/dev` | The original project's development line, including unreleased changes. |
| `dev` / `origin/dev` | Upstream development tracking; keep custom features on separate branches. |

A fresh clone needs the upstream remote for synchronization:

```sh
git remote add upstream https://github.com/clash-verge-rev/clash-verge-rev.git
git fetch upstream
```

Create a synchronization branch from the custom client's `main`, then merge the chosen upstream line and review conflicts and behavior there:

```sh
git switch main
git switch -c codex/sync-upstream
git merge --no-commit --no-ff upstream/main
```

Use `upstream/dev` when an unreleased fix is needed. Verify application routing, Freecat subscriptions and DNS overrides before submitting the synchronization PR to `main`. Keep project-specific documentation, release links and core patches aligned with the custom client.

Code, comments, commits and PR text use English. Commit subjects follow Conventional Commits. Agent-assisted PRs append an `Assisted by:` footer with the model and available effort information. The PR describes the problem, why the approach solves it and the scope; see [the template](./.github/PULL_REQUEST_TEMPLATE.md).

The existing [AI contribution review](./.github/workflows/pr-ai-slop-review.md) remains compiled with `gh aw compile`; changing its policy also requires regenerating the lock file.

## Prerequisites

- Node.js; CI currently uses Node 24.
- The pnpm version declared in `package.json`.
- The Rust toolchain declared in `rust-toolchain.toml`.
- Go, curl, tar and Git for rebuilding the application-routing core.
- [Tauri platform prerequisites](https://tauri.app/start/prerequisites/).

Windows builds use Rust's MSVC target and the Windows SDK/build tools. Windows ARM builds also require LLVM/clang for dependencies that use `ring`.

## Prepare and run

```sh
git clone --branch main https://github.com/Avalon0409/clash-verge-rev.git
cd clash-verge-rev
pnpm install
pnpm prebuild
pnpm build:process-core
pnpm dev
```

`pnpm prebuild` prepares platform resources and the service. `pnpm build:process-core` then downloads the pinned Mihomo source, verifies its checksum, applies the project patch, runs its regression packages and builds the bundled core. Run the core build again after forcing a resource refresh with `pnpm prebuild --force`.

`pnpm dev` preserves the isolated development environment's existing service installation. Use `pnpm dev:service` to install or update the development service explicitly. Use `pnpm dev:sidecar` to run without that service. `pnpm web:dev` runs the frontend without the desktop backend.

## Build custom packages

On macOS:

```sh
pnpm build:app-proxy
```

This rebuilds the patched core and produces a debug application bundle with `tauri.app-proxy.conf.json`. That configuration requires macOS 12 or later and disables automatic application updates.

On Windows:

```sh
pnpm build:process-core
pnpm exec tauri build --config src-tauri/tauri.app-proxy.conf.json
```

The Windows core build also updates the installer's bundled-core hashes. Select a target matching the installed Rust and Node architecture when cross-building.

## Validate a change

Use checks relevant to the affected area:

```sh
pnpm typecheck
pnpm lint
pnpm arch:check
pnpm test
cargo fmt --check
cargo clippy-all
```

Run focused tests for the affected code before broader suites. Network and platform-specific tests need their corresponding services or native host; describe the actual coverage in the PR. [Changelog.md](./Changelog.md) records user-visible changes according to [the release-note rules](./template/Changelog.md).

## Translations

See [CONTRIBUTING_i18n.md](./docs/CONTRIBUTING_i18n.md) for frontend and backend translation ownership. The Chinese and English READMEs describe the custom client; other language landing pages link to those maintained guides.
