# Clash Verge App Proxy user guide

## Installation and updates

Download a matching `app-proxy` release from [this repository](https://github.com/Avalon0409/clash-verge-rev/releases). macOS requires version 12 or later. Choose the Windows installer architecture shown in the release assets.

Custom releases use manual updates. You can export configuration or use WebDAV backup before installing a new package. After installation, verify that the matching bundled core is loaded in the core settings, then check application routing.

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

Passwords, login sessions and decryption material use macOS Keychain or Windows Credential Manager. The service address, email and credential reference are stored with the profile. Sign in again if the session or system credential store is unavailable.

## DNS overrides

Global DNS overrides apply to the shared configuration. Subscription-level overrides apply to the corresponding independent subscription. An application's selected subscription participates in generating its DNS settings. Check both the saved values and running core configuration after changing DNS.

Application DNS egress and native Windows multi-adapter routing changes are under review in [PR #1](https://github.com/Avalon0409/clash-verge-rev/pull/1). Reports for those problems should include global and application node choices, DNS settings and reproduction steps.

## Backups and privacy

Export configuration or configure WebDAV backup. You choose where to store the profiles and proxy configuration. Account secrets live in the system credential store, so sign in again when moving to another computer.

See [PRIVACY.md](../PRIVACY.md) for storage, logs and network data flows.

## Help

Read the [FAQ](./faq_en.md), then search [this repository](https://github.com/Avalon0409/clash-verge-rev) for an existing report. Include the release tag, operating system, routing mode and reproduction steps.
