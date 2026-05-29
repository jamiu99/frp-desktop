# frp-desktop

[![release](https://github.com/jamiu99/frp-desktop/actions/workflows/release.yml/badge.svg)](https://github.com/jamiu99/frp-desktop/actions/workflows/release.yml)
[![license: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

跨平台的 [frp](https://github.com/fatedier/frp) 图形化客户端。把 `frpc.toml` 这种命令行工具变成图形化、有命名约束、能管多个 frps 服务端的桌面应用。

> 适用于 Windows / macOS / Linux。基于 Tauri 2 + Vue 3 + TypeScript。

## 为什么要做这个

直接用 frp 官方 CLI，常见问题：

1. 多个 proxy 写在一个 `frpc.toml` 里，**启停粒度粗**——要么全开要么全关
2. 用户随手起 `test1`、`a`、`temp` 这种 **没意义的 name**，几个月后无法维护
3. **多 frps 服务端切换** 麻烦，每次要改配置
4. **frps dashboard 在浏览器里看**，每次输 URL 输密码

frp-desktop 解决：

- ✅ 多 proxy 独立启停，每个 proxy 一个独立的 frpc 子进程，互不影响
- ✅ **强约束 name 管理**：必须挂在「项目」下，name = `{项目}-{用途}` 自动拼接，黑名单常见占位词，重名检查，必填描述
- ✅ 多 frps 服务端配置，一处管理
- ✅ 应用内查看 frps dashboard（serverinfo + proxy 列表 + 流量统计）
- ✅ 系统托盘常驻、关窗口不退出、开机自启
- ✅ 顺带提供本机端口查看器（进程 / PID / 协议 / 地址 / 端口 / 状态，可排序过滤）

## 截图

> 截图待补。可在 [Releases](https://github.com/jamiu99/frp-desktop/releases) 下载安装包后体验。

## 安装

### 直接下载

到 [Releases](https://github.com/jamiu99/frp-desktop/releases) 下载对应平台的安装包：

| 平台 | 文件 |
| --- | --- |
| Windows | `.msi` 或 `.exe` |
| macOS (Apple Silicon) | `*_aarch64.dmg` |
| macOS (Intel) | `*_x64.dmg` |
| Linux | `.AppImage` 或 `.deb` |

frpc 二进制（v0.69.0）已随包内置，**无需另外下载**。

### 从源码构建

需要：Node.js 20+ / pnpm 9+ / Rust 1.78+。

```bash
git clone https://github.com/jamiu99/frp-desktop.git
cd frp-desktop
pnpm install

# 下载 frpc sidecar（首次构建必需，已锁定版本 0.69.0）
bash src-tauri/binaries/fetch.sh

# 开发模式
pnpm tauri dev

# 打包发布
pnpm tauri build
```

#### Linux 额外系统依赖

```bash
sudo apt install -y \
  libwebkit2gtk-4.1-dev libsoup-3.0-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libxdo-dev \
  patchelf build-essential pkg-config
```

## 使用

1. **添加 frps 服务端**：填 host、port、token；可选填 dashboard URL + 用户名密码
2. **创建项目**：例如 `nas`、`company-vpn`、`home-iot`（小写字母/数字/连字符，2~32 字符）
3. **在项目下添加 Proxy**：
   - 选项目和服务端
   - 填用途（如 `ssh`、`webui`），name 会自动拼成 `{项目}-{用途}`
   - 必填描述（≥10 字）
   - 配置类型 / 本地端口 / 远端端口 / 域名等
4. **启动 Proxy**：点「启动」，frpc 子进程运行，可点「日志」查看实时输出
5. **查看 dashboard**：在「frps 服务端」页点对应服务端的「查看 dashboard」

## 命名约束（这是本应用的核心设计）

为了避免 `test1`、`a`、`temp` 这种没意义的 proxy 名造成长期维护灾难，强制：

- 所有 Proxy 必须挂在某个 Project 下
- name = `{project}-{purpose}`，前缀来自项目名，自动拼接
- 项目名和用途都受正则约束（小写字母/数字/连字符）
- 黑名单：拒绝 `test`、`test1`、`temp`、`a`、`b`、`1`、`xxx`、`demo`、`foo`、`bar` 等占位词
- 同一 frps 服务端内 name 查重
- 必填描述（≥10 字符）

## 架构

- **桌面外壳**：Tauri 2（Rust）
- **前端**：Vue 3 + TypeScript + Tailwind v3 + shadcn-vue 风格组件
- **本地存储**：单 JSON 文件，路径 `app_data_dir/store.json`，原子写入
- **frpc 集成**：调用官方 frpc 二进制（sidecar 模式，随包分发，锁定 v0.69.0）。每个启用的 proxy 一个独立的 frpc 子进程，独立 toml 配置文件
- **frps 接入**：调用 frps Admin HTTP API（`/api/serverinfo`、`/api/proxy/{type}`）

详细架构决策见 [docs/STATUS.md](docs/STATUS.md)。

## 数据存储位置

| 平台 | 路径 |
| --- | --- |
| Windows | `%APPDATA%\com.github.jamiu99.frp-desktop\` |
| macOS | `~/Library/Application Support/com.github.jamiu99.frp-desktop/` |
| Linux | `~/.config/com.github.jamiu99.frp-desktop/` |

⚠️ **注意**：当前版本 frps token 和 dashboard 密码以**明文**存在 `store.json`。文件权限依赖操作系统的用户目录隔离。如有更高安全要求，请勿在多用户环境下使用。

## 升级 frpc

```bash
FRP_VERSION=0.70.0 bash src-tauri/binaries/fetch.sh
```

会自动下载、sha256 校验、按 Tauri sidecar 命名约定放置全部 6 个 target triple 的二进制。

## 开发

```bash
# 启动 dev 环境（tmux 后台跑 tauri dev + vite，含 frpc sidecar）
./start.sh start
./start.sh logs       # 实时日志
./start.sh stop
./start.sh restart
```

## License

[MIT](LICENSE) © 2026 jamiu99

frp-desktop 内置的 frpc 二进制 © [fatedier/frp](https://github.com/fatedier/frp)，遵循 frp 项目自身的 Apache-2.0 License。
