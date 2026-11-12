<h1 align="center">
  <img src="./src-tauri/icons/icon.png" alt="Clash Verge App Proxy" width="128" />
  <br />
  Clash Verge App Proxy
</h1>

<p align="center">面向 Windows 和 macOS 的应用与进程代理客户端，支持独立订阅、Freecat 账号和 DNS 覆盖。</p>

<p align="center">
  <a href="https://github.com/Avalon0409/clash-verge-rev/tree/main">项目源码</a> ·
  <a href="https://github.com/Avalon0409/clash-verge-rev/releases">下载安装</a> ·
  <a href="./docs/usage.md">使用说明</a> ·
  <a href="./docs/faq.md">常见问题</a> ·
  <a href="./docs/README_en.md">English</a>
</p>

Clash Verge App Proxy 是本仓库独立维护的 Clash Verge Rev 派生版本。我们在原有代理客户端的基础上，加入了应用与进程分流、应用独立订阅、Freecat 账号订阅及全局和订阅级 DNS 覆盖。项目说明、发行包和问题反馈均由本仓库维护。

## 安装与更新

从 [本项目 Releases](https://github.com/Avalon0409/clash-verge-rev/releases) 下载带有 `app-proxy` 标识的发行包，并按附件中的操作系统和架构选择安装文件。

| 平台 | 安装说明 |
| --- | --- |
| Windows | 使用匹配系统架构的安装包；启用 TUN 时按客户端提示安装或启动服务。 |
| macOS | 需要 macOS 12 或更新版本；选择匹配 Intel 或 Apple Silicon 的安装包。 |

本项目的自定义发行包采用手动更新。升级时从上述 Releases 下载新版；应用分流需要与客户端配套的内置核心，可以在核心设置中重新加载内置核心。具体操作见 [安装与更新说明](./docs/usage.md#安装与更新)。

## 功能

- 按应用选择规则分流或强制代理，并保存各自的订阅和节点。
- 显示应用、父子进程、连接和实时流量，支持进程临时覆盖及恢复继承配置。
- 在 Windows 发现开始菜单中的应用，在 macOS 发现 `.app` 应用并覆盖包内子进程。
- 使用 Freecat 账号登录并导入、更新订阅；账号秘密保存在 macOS Keychain 或 Windows Credential Manager 中。
- 设置全局 DNS 覆盖和独立订阅的 DNS 覆盖。
- 管理系统代理、TUN、节点、规则、订阅增强、主题和 WebDAV 备份。

应用代理打开后，未配置和已关闭代理的 TUN 应用默认使用系统网络。系统代理、应用代理和进程临时设置的作用范围见 [使用说明](./docs/usage.md)。

Windows 按完整 EXE 路径保存应用配置。点击刷新会清理已不存在的应用及其保存配置；新版 EXE 可以重新选择和配置。现有继承覆盖父子进程。详见 [Windows 应用更新](./docs/faq.md#windows-application-updates)。

## 开发

本项目以 `main` 为主分支，功能和修复通过独立分支开发。准备 Node.js、项目指定的 pnpm、Rust 工具链、Go，以及对应平台的 Tauri 编译依赖后执行：

```sh
git clone --branch main https://github.com/Avalon0409/clash-verge-rev.git
cd clash-verge-rev
pnpm install
pnpm prebuild
pnpm build:process-core
pnpm dev
```

`pnpm build:process-core` 会校验并重建本项目的应用分流核心。使用 `pnpm prebuild --force` 后应重新运行它。

`pnpm dev` 使用独立开发环境，保留该环境已有的服务安装状态。需要主动安装或更新开发服务时运行 `pnpm dev:service`；需要 Sidecar 模式时运行 `pnpm dev:sidecar`。

构建与贡献说明见 [CONTRIBUTING.md](./CONTRIBUTING.md)。

## 反馈

在 [本项目已有反馈](https://github.com/Avalon0409/clash-verge-rev/issues) 中查找相同问题。Issues 关闭时，维护者可以通过 [Pull requests](https://github.com/Avalon0409/clash-verge-rev/pulls) 提供问题说明和修复提案。请附上客户端版本、发行包标签、系统版本和复现步骤，提交前移除日志中的账号秘密和订阅凭据。

## 隐私与许可

配置、应用分流设置和日志保存在本机。账号登录、订阅更新、DNS、测速和 WebDAV 备份会联系对应服务；具体数据流与凭据存储方式见 [隐私说明](./PRIVACY.md)。

本项目沿用 Clash Verge Rev、Clash Verge 及其贡献者的来源署名，基于 Tauri 和 Mihomo 构建，使用 GPL-3.0-only 许可。原有版权声明与许可证保留在源码和 [LICENSE](./LICENSE) 中。

其他语言： [Español](./docs/README_es.md) · [فارسی](./docs/README_fa.md) · [日本語](./docs/README_ja.md) · [한국어](./docs/README_ko.md) · [Português](./docs/README_pt.md) · [Русский](./docs/README_ru.md)
