# frp_desktop 项目状态

> 本文档持续维护项目当前状态。每完成一项功能、每做一次重要决策、每修复值得记录的 bug，都在此处更新。
> 阅读对象：几个月后回来看的自己 / 接手项目的人。重在 **why**，不只是 **what**。

最后更新：2026-05-25

---

## 1. 项目目标

frp_desktop 是一个跨平台（Windows / macOS / Linux）的桌面应用，定位是**比官方 frpc 更易用的图形化客户端**，并且**附带 frps 服务端的查看功能**。

核心痛点：
1. 官方 frpc 是命令行 + toml 配置，对非工程师用户不友好
2. 多个 proxy 配在一个 frpc.toml 里，启停粒度粗（要么全开要么全关）
3. 用户经常给 proxy 起 `test1`、`a`、`temp` 这种没意义的 name，时间一长就忘了哪个是哪个，多人协作时尤其乱
4. 官方 frps dashboard 在浏览器里看，每次都要输 URL 和密码，不方便

本项目要解决：
- 图形化管理 frpc 客户端配置和运行
- **强约束的 name 管理**：项目（Project）分组 + 命名规则 + 黑名单 + 查重 + 必填描述
- 内嵌 frps dashboard 查看能力，多服务端切换
- 系统托盘常驻、开机自启、多 proxy 独立启停

## 2. 技术栈

| 层级 | 选型 | 原因 |
|---|---|---|
| 桌面外壳 | **Tauri 2** | 包小（~10MB）、内存占用低、生态在快速成熟、跨平台原生 WebView |
| 前端框架 | **Vue 3 + TypeScript** | 用户熟悉度 + 生态成熟 |
| 构建工具 | **Vite** | Tauri 默认 |
| 包管理 | **pnpm** | 全局规范 |
| UI 组件库 | 待定（候选：shadcn-vue + Tailwind / Element Plus / Naive UI） | 需要好看 + 浅色模式默认 |
| 本地存储 | **SQLite**（tauri-plugin-sql 或 rusqlite） | 全局规范偏好 sqlite，加密字段（dashboard 密码、frps token）单独处理 |
| frpc 集成 | **官方 frpc 二进制 + sidecar 分发** | 与官方版本兼容，升级简单。打包时把三平台 frpc 放进 `src-tauri/binaries/` |
| frps 接入 | **HTTP API**（frps Admin API） | 直接调 `/api/proxy/{type}` 等端点拉数据 |
| CI/CD | **GitHub Actions** | 三平台矩阵自动构建 |

## 3. 关键架构决策

### ADR-001：用 Tauri 不用 Electron / Wails / Flutter
- **Electron**：包大、内存重，与"轻量桌面客户端"定位不符
- **Wails**：Go 生态可复用 frp 库，但生态偏小、Windows 下还有些坑
- **Flutter**：要学 Dart、桌面端组件偏移动风格、UI 定制不如 Web
- **Tauri 2**：包小、安全模型清晰、Web 前端任意选

### ADR-002：调用 frpc 二进制而不是嵌入 frp Go 库
- 嵌入 Go 库需要 cgo + 锁定 frp 版本，升级成本高
- 调用二进制：生成 toml → 启动 frpc 子进程 → 通过 stdout/stderr 监控状态
- 二进制随包分发（sidecar），用户零配置

### ADR-003：强约束的 Name 管理
**问题**：用户随手起 `test1`、`a` 这类 name，N 个 proxy 之后无法维护。

**约束**（多层防线）：
1. **必须挂在 Project 下**：先建项目（如 `nas`、`company-vpn`），再加 proxy
2. **格式校验**：name 形如 `{project}-{purpose}`，自动从两个字段拼接（如 `nas-ssh`、`nas-webui`），用户填的是 `purpose`
3. **黑名单**：拒绝 `test`、`test1`、`temp`、`a`、`b`、`1`、`xxx`、`demo`、`tmp` 等占位词作为 purpose
4. **同 frps 内查重**：同一 frps 服务端下不允许重名 proxy
5. **必填描述**：每个 proxy 强制写一句"这是干什么的"（>= 10 字符）

### ADR-004：每个项目都有 `docs/STATUS.md`
- 全局 CLAUDE.md 已写入此规范
- 每完成功能/做决策/修 bug 都要更新

## 4. 已实现功能

| 日期 | 功能 | 备注 |
|---|---|---|
| 2026-05-25 | Rust 工具链 + Tauri Linux 系统依赖安装 | rustc 1.95、webkit2gtk 4.1 |
| 2026-05-25 | Tauri 2 + Vue 3 + TS 脚手架初始化 | `pnpm create tauri-app` |
| 2026-05-25 | docs/STATUS.md 初版 | 立下文档规范 |
| 2026-05-25 | UI 骨架（侧栏 + 顶栏 + 5 个空白页面） | shadcn-vue + Tailwind v3，浅色默认 |
| 2026-05-25 | start.sh：tmux 管理 dev 环境 | start/stop/restart/status/attach/logs/web |
| 2026-05-25 | 字号微调：根字号 16 → 17 | 中文在 webview 里偏小 |
| 2026-05-25 | 本机端口查看器（PortsView） | netstat2 + sysinfo，6 列可排序 + 过滤 |
| 2026-05-25 | 字号 17 → 18 | 桌面 webview 中文阅读舒适度 |
| 2026-05-25 | bundle identifier 改为 com.github.jamiu99.frp-desktop | 用户决定 |
| 2026-05-25 | 数据模型 + JSON 单文件存储 | Project / FrpsServer / Proxy / Settings；前后端类型同步；Pinia store；强 name 校验已落地 |
| 2026-05-27 | UI 通用组件：Dialog / Textarea / NativeSelect / Toast / Confirm | shadcn-vue 风格，全部手写 |
| 2026-05-27 | frps 服务端管理（CRUD + Dashboard 集成） | ServerFormDialog；DashboardView 调 frps Admin API（/api/serverinfo + /api/proxy/{type}），表格 + 排序 |
| 2026-05-27 | 项目 CRUD + Proxy CRUD + name 规则 UI | ProjectFormDialog / ProxyFormDialog；name = {project}-{purpose} 实时拼接预览，黑名单 / 重名检查实时提示 |
| 2026-05-27 | frpc 子进程管理 | frpc.rs：每个 proxy 一个子进程，独立 toml 配置（{proxy_id}.toml），stdout/stderr 实时推送给前端，最近 500 行环形日志 |
| 2026-05-27 | Proxy 启停 + 日志查看 | ProxiesView 启动/停止按钮，状态徽标（running/starting/crashed/stopped），ProxyLogsDialog 实时日志 |
| 2026-05-27 | 设置页：frpc 路径 + close-to-tray + 自启 | 检查 frpc 按钮可一键验证版本和路径 |
| 2026-05-27 | 系统托盘 + 关窗口不退出 + 自启 | 托盘菜单（显示主窗口/退出），左键点托盘呼起窗口；close_to_tray=true 时关闭按钮最小化；autostart 通过 plugin |
| 2026-05-27 | GitHub Actions 三平台 release 工作流 | tag push 触发；matrix 构建 Win x64 / macOS Intel + Apple Silicon / Linux x64；草稿 release |
| 2026-05-29 | 锁定 frpc v0.69.0 + sidecar 全平台二进制 | 6 个 target triple（linux x64/arm64、macOS Intel/AS、windows x64/arm64）放 `src-tauri/binaries/`，配 `bundle.externalBin`，dev 模式 cargo 自动拷到 target/debug/。配套 `binaries/fetch.sh` 一键升级（含 sha256 校验）。frpc.rs 查找路径优先用 sidecar，零配置可用 |
| 2026-05-30 | v0.1.1 重构：去掉 Project，进程按 server 聚合 | 用户反馈 Project 多余 + 描述限制反人类。删 Project 模型、去掉黑名单和 ≥10 字校验；name 用户自填，仅校验格式 + 同 server 唯一。frpc 进程模型从"每 proxy 一个进程"改为"每 server 一个进程"（共享 toml，热重启）。Windows 默认隐藏 frpc 控制台（CREATE_NO_WINDOW），加 settings.show_frpc_console 开关。进程崩溃/手动关掉黑窗后 UI 自动切到「已崩溃」。状态文本全部中文化 |

## 5. 进行中

（无）核心功能已闭环。下一步是用户实际使用反馈驱动迭代。

## 6. 待办 / 下一阶段候选

主线功能已完成。后续视使用反馈做：

- **端口查看器联动**：选中行右键 → 复制端口 / 用此端口创建 frpc proxy（跳到新建表单并预填本地端口）
- **端口查看器轮询刷新**：开关式，避免一直占 CPU
- **frpc 配置高级字段**：bandwidth_limit、压缩、加密、健康检查等
- **Project 颜色标签**：UI 上用颜色快速区分项目
- **Proxy 拷贝/导出**：导出标准 frpc.toml 给手动场景用
- **Dashboard 实时刷新**：当前是手动点刷新

## 7. 已知问题

- 暂无

## 8. 参考资料

- Tauri 2 文档：https://tauri.app/
- frp 官方仓库：https://github.com/fatedier/frp
- frps Admin API：https://github.com/fatedier/frp/blob/dev/doc/server_api.md
- 测试用 frps（示例）：http://frps.example.com:5002/static/#/proxies/tcp
