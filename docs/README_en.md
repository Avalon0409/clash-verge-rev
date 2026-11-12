<h1 align="center">
  <img src="../src-tauri/icons/icon.png" alt="Clash Verge App Proxy" width="128" />
  <br />
  Clash Verge App Proxy
</h1>

<p align="center">Application and process routing for Windows and macOS, with independent subscriptions, Freecat accounts and DNS overrides.</p>

<p align="center">
  <a href="https://github.com/Avalon0409/clash-verge-rev/tree/main">Source</a> ·
  <a href="https://github.com/Avalon0409/clash-verge-rev/releases">Downloads</a> ·
  <a href="./usage_en.md">User guide</a> ·
  <a href="./faq_en.md">FAQ</a> ·
  <a href="../README.md">简体中文</a>
</p>

Clash Verge App Proxy is an independently maintained derivative of Clash Verge Rev. This repository adds application and process routing, separate subscriptions per application, Freecat account subscriptions, and global and subscription-level DNS overrides. This repository maintains the documentation, packages and feedback for this version.

## Installation and updates

Download a release marked `app-proxy` from [this project's Releases](https://github.com/Avalon0409/clash-verge-rev/releases). Select the operating system and architecture shown in the release assets.

| Platform | Installation |
| --- | --- |
| Windows | Choose the matching installer. Install or start the service when the client prompts you to enable TUN. |
| macOS | Requires macOS 12 or later. Choose the matching Intel or Apple Silicon package. |

Custom releases use manual updates. Download a new package from this repository when upgrading. Application routing requires the matching bundled core; the core settings can reload it. See the [installation guide](./usage_en.md#installation-and-updates).

## Features

- Save an application's routing mode, subscription and selected node.
- Inspect applications, parent and child processes, connections and traffic; apply temporary process settings and restore inherited settings.
- Discover Start Menu applications on Windows and `.app` bundles on macOS.
- Sign in to a Freecat account and import or refresh its subscription. Account secrets use macOS Keychain or Windows Credential Manager.
- Configure global and per-subscription DNS overrides.
- Manage system proxy, TUN, nodes, rules, subscription enhancements, themes and WebDAV backups.

With application routing enabled, unconfigured and disabled TUN applications default to native system networking. See the [user guide](./usage_en.md) for how system proxy and process overrides interact with application routing.

Windows application settings use the full EXE path. Refreshing the list removes applications and saved settings whose paths no longer exist. Select and configure a new EXE after an upgrade changes its path. Existing inheritance follows parent and child processes. See [Windows application updates](./faq_en.md#windows-application-updates).

## Development

The custom client is maintained on `main`; features and fixes use separate working branches. Install Node.js, the pnpm version declared by the project, its Rust toolchain, Go and the platform's Tauri prerequisites, then run:

```sh
git clone --branch main https://github.com/Avalon0409/clash-verge-rev.git
cd clash-verge-rev
pnpm install
pnpm prebuild
pnpm build:process-core
pnpm dev
```

`pnpm build:process-core` verifies and rebuilds this project's application-routing core. Run it again after `pnpm prebuild --force`.

`pnpm dev` preserves the isolated development environment's existing service state. Use `pnpm dev:service` to install or update that service explicitly, or `pnpm dev:sidecar` to use Sidecar mode.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for build and contribution details.

## Feedback

Search [this repository's issues](https://github.com/Avalon0409/clash-verge-rev/issues) for an existing report. When Issues are disabled, maintainers can record a problem and proposed fix in [Pull requests](https://github.com/Avalon0409/clash-verge-rev/pulls). Include the client version, release tag, operating system and reproduction steps. Remove account secrets and subscription credentials from attachments.

## Privacy and license

Configuration, routing settings and logs are stored locally. Account sign-in, subscription refreshes, DNS, latency tests and WebDAV backups contact their respective services. See the [privacy policy](../PRIVACY.md) for data flows and credential storage.

This project retains attribution to Clash Verge Rev, Clash Verge and their contributors. It uses Tauri and Mihomo and is licensed under GPL-3.0-only. Existing copyright notices and [LICENSE](../LICENSE) remain part of the source.

Other languages: [Español](./README_es.md) · [فارسی](./README_fa.md) · [日本語](./README_ja.md) · [한국어](./README_ko.md) · [Português](./README_pt.md) · [Русский](./README_ru.md)
