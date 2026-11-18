# Clash Verge App Proxy user guide

## Installation and updates

Download a matching `app-proxy` release from [this repository](https://github.com/Avalon0409/clash-verge-rev/releases). macOS requires version 12 or later. Choose the Windows installer architecture shown in the release assets.

Starting with 2.5.13, manual and automatic update checks use this repository’s GitHub Releases and verify the project’s package signature. Install 2.5.13 manually once from Releases when upgrading from 2.5.12 or earlier, then use in-app updates.

Packages contain the patched core, service and required resources. The core updates with the application; core settings provide restart and use-bundled-core actions. Startup backs up old configuration, recreates missing enhancement files, restores available configuration and subscription caches, and synchronizes service core copies when they differ from the package. Approve the operating system’s administrator prompt if synchronization requires it. Application data remains outside the installed bundle, preserving accounts, subscriptions, node selections and application routing.

On macOS, replace `/Applications/Clash Verge App Proxy.app` and launch that copy. The client registers its launch entry again. Development bundles use `target.noindex` to keep them out of application search.

## Application routing

1. Add or import a subscription and confirm that its nodes are available.
2. Open application routing, enable it and prepare TUN and the service when prompted.
3. Select an application from the list, or add a `.app` bundle or EXE manually.
4. Choose its subscription, routing mode and node, then enable the application.
5. Inspect the application's processes, connections and traffic.

Rule mode uses the selected subscription's rules and can select the application's proxy node. Direct and reject rules retain their behavior. Forced proxy mode uses the selected node. Select a new target if the subscription or node is missing.

Unconfigured and disabled TUN applications default to native networking. System proxy separately affects programs that honor its settings; disable system proxy when checking application-only routing.

## Process inheritance

Application settings persist. Child processes search their parent chain for settings; macOS bundle settings also cover processes inside the bundle. A temporary process override belongs to that running process instance. Resetting it restores the inherited application or parent settings.

Windows application settings use the full EXE path. The refresh button checks saved paths, removes missing applications and their settings, and scans installed application entries again. Select and configure the new executable after an upgrade changes its path. Process snapshots continue to refresh periodically. An inspection error stops cleanup and retains the original configuration. See [Windows application updates](./faq_en.md#windows-application-updates).

## Freecat account subscriptions

Create a Freecat account subscription on the profiles page, enter your HTTPS service address, account and password, and sign in to import it. You can refresh the account subscription later.

Passwords, login sessions and decryption material use private local files under `account-cache/` in the application data directory. Routine sign-in and refresh no longer use Keychain or Credential Manager. The first upgrade attempts a noninteractive migration of accessible legacy credentials; sign in once if they cannot be migrated. The service address, email and credential reference remain with the profile.

## DNS overrides

Global DNS overrides apply to the shared configuration. Subscription-level overrides apply to the corresponding independent subscription. An application's selected subscription participates in generating its DNS settings. Check both the saved values and running core configuration after changing DNS.

Version 2.5.12 includes the application DNS egress and native Windows multi-adapter routing fixes from [PR #1](https://github.com/Avalon0409/clash-verge-rev/pull/1). Reports for those problems should include global and application node choices, DNS settings and reproduction steps.

## Backups and privacy

Export configuration or configure WebDAV backup. You choose where to store the profiles and proxy configuration. Account cache files remain local and are separate from exported profiles, so sign in with your own account when moving to another computer.

See [PRIVACY.md](../PRIVACY.md) for storage, logs and network data flows.

## Help

Read the [FAQ](./faq_en.md), then search [this repository](https://github.com/Avalon0409/clash-verge-rev) for an existing report. Include the release tag, operating system, routing mode and reproduction steps.
