# Clash Verge App Proxy FAQ

## Windows application updates

Windows identifies an application by its full EXE path. When an installer creates a new version directory, the old and new paths become separate entries. The refresh button removes saved entries whose executables no longer exist and scans installed application entries again. An old EXE that remains on disk keeps its entry.

Record the old entry's enabled state, routing mode, subscription and node before refreshing if you want to reuse them. Refresh the list, select the new EXE and configure it with those settings. Cleanup removes the saved settings for missing paths. A path-inspection error stops cleanup and retains the original configuration.

Current inheritance follows parent and child processes. Upgrade inheritance based on a stable installation identity has not been added to this version.

## macOS application updates

macOS usually replaces a `.app` bundle at the same path, such as `/Applications/Example.app`. The stable bundle path continues to match its main and helper processes. Select the new path if you move, rename or reinstall the bundle into another directory.

## Service core permissions

Check the service state in settings and use the client's install or repair action. Follow any data-directory ownership or permission instructions shown by the service check. If another application owns the core process, stop that application before starting this client's core. Use the operation and error in the logs to decide what to repair.

## Other programs still use a proxy

Check whether system proxy is enabled or the program has its own HTTP/SOCKS proxy configuration. Disable those settings when testing application-only routing, then inspect the actual process and node on the connections page.

## Missing application routing after an upgrade

Install an `app-proxy` package from [this repository](https://github.com/Avalon0409/clash-verge-rev/releases). Version 2.5.13 checks and synchronizes the matching core; approve the operating system’s administrator prompt when required. Separate core upgrades are disabled; the patched core updates with the application package.

## Freecat login or refresh fails

Check the HTTPS service address, account and password, then sign in again. Starting with 2.5.13, passwords and sessions use private local cache files. Routine operations no longer access Keychain or Credential Manager. The first upgrade attempts a silent migration; sign in once if the old credentials are inaccessible. Install the package and sign in with your own account on another computer. Include the version and error when reporting a service protocol change.

## Why does a macOS upgrade still ask for a password?

Installing or updating the TUN service, or publishing bundled cores into its system directory, requires macOS administrator authentication. Startup reuses the existing copies when both the service and cores match the package. Account sign-in and subscription refresh use the local cache. Legacy Keychain migration suppresses authentication dialogs; sign in once in the client if silent migration cannot read the old credentials.

## Unexpected application DNS egress

Check the application's subscription and DNS overrides, compare global and application nodes, and record the actual exit shown by the connections page or logs. Version 2.5.12 includes the application DNS egress and native Windows multi-adapter routing fixes from [PR #1](https://github.com/Avalon0409/clash-verge-rev/pull/1).

## Feedback

Use [this repository's issues](https://github.com/Avalon0409/clash-verge-rev/issues). When Issues are disabled, maintainers can record reports and proposals in [Pull requests](https://github.com/Avalon0409/clash-verge-rev/pulls). Include the release tag, operating system and reproduction steps. Remove passwords, sessions and subscription credentials from attachments.
