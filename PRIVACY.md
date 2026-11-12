# Clash Verge App Proxy privacy policy

**Applies to:** the Windows and macOS custom releases maintained in [Avalon0409/clash-verge-rev](https://github.com/Avalon0409/clash-verge-rev).

**Last updated:** 2026-11-12

Clash Verge App Proxy stores application configuration and logs locally. Features that contact a network service include proxy traffic, Freecat sign-in, subscription downloads, DNS, network tests and backups. This document describes those connections and the data they use.

## Local configuration and credentials

The custom client uses the existing application data directory:

| Platform | Location |
| --- | --- |
| Windows | `%APPDATA%\io.github.clash-verge-rev.clash-verge-rev` |
| macOS | `~/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev` |

Application settings include selected application paths, routing modes, subscriptions and nodes. Profile files can contain subscription URLs, proxy addresses and credentials. Core caches, DNS configuration and logs also live on the device. WebDAV connection details are stored with the application settings.

Freecat passwords, login sessions and subscription decryption material use **macOS Keychain** or **Windows Credential Manager**. Profile metadata stores the account's service address, email and a reference to those credentials. Copying application files to another computer requires signing in there. Remove an account through the client to remove its associated stored credentials.

Logs can contain application paths, process information, requested domains, IP addresses and connection errors. The log level and cleanup settings control what is retained. Exported configurations and logs may include sensitive account or subscription information.

## Network connections

### Proxy traffic

The bundled Mihomo core sends routed traffic to the proxy servers selected in your configuration. Direct traffic uses the system network. The selected proxy and destination services govern their own data handling.

### Freecat accounts

Signing in sends your account email and password over HTTPS to the service address you provide. Subscription refreshes use the returned login session and download subscription data from that provider. The client stores account secrets in the operating system's credential store and decrypts the returned subscription locally.

The account provider operates this login and subscription service. Its privacy policy applies to the information you submit to it.

### Remote subscriptions

Adding or refreshing a remote profile contacts its subscription URL. The provider receives normal HTTP request information, such as your source IP address and User-Agent, along with any credentials included in the URL. A profile's automatic refresh uses the interval you configure.

### DNS

DNS resolution contacts the resolvers used by your system and generated core configuration. Global or subscription-level DNS overrides can change those resolvers. They receive the domain names being resolved. Application-routing mode selects DNS settings after identifying the application connection.

### Updates and downloads

Custom `app-proxy` releases use manual application updates from [this project's Releases](https://github.com/Avalon0409/clash-verge-rev/releases). The client has no upstream application-update endpoint configured. GitHub receives normal requests when you open the release page or download a package.

Application routing uses the matching bundled core. Developer resource preparation and user-initiated component downloads can contact component providers, including Mihomo and the service component's release host.

### IP information and network tests

The home IP card contacts third-party IP and geolocation services while active. The configured service receives the source IP of its request. Latency tests contact their configured test URL through the tested proxy. Streaming and AI availability tests contact the services being tested when you start them.

These destinations are independent third parties. The client also supports opening external dashboards, which load in your browser and connect to the local core.

### WebDAV and exported backups

WebDAV backup uploads configuration to the server you choose. Backup archives can include subscription URLs and proxy credentials. Account secrets in Keychain or Credential Manager are separate from the profile files. You control local exports and the destination of backup uploads.

### Local control and proxy listeners

The core exposes local control and proxy listeners according to its runtime configuration. Enabling LAN access changes which devices can reach a proxy listener; check the runtime configuration when configuring network access.

## Your controls

You can change application routing, system proxy, DNS settings, subscription refresh intervals, network-test destinations and WebDAV settings. You can disable the home IP card and configure log cleanup. To remove a Freecat account, remove it through the client; to remove file-based configuration, delete the applicable data files after making any backup you want to retain.

## Project services and reporting

This fork adds no analytics or telemetry endpoint. Account providers, DNS providers, proxies, GitHub and backup servers handle the requests described above under their own policies.

Use [this repository](https://github.com/Avalon0409/clash-verge-rev) for questions and reports. Remove passwords, sessions, subscription credentials and private configuration from public attachments. Changes to this document are tracked in the repository history.
