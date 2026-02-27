# BrowserCLI

[English](./README.md) | **中文**

[![CI](https://github.com/justinhuangcode/browsercli/actions/workflows/ci.yml/badge.svg)](https://github.com/justinhuangcode/browsercli/actions/workflows/ci.yml)
[![Release](https://github.com/justinhuangcode/browsercli/actions/workflows/release.yml/badge.svg)](https://github.com/justinhuangcode/browsercli/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/browsercli?style=flat-square)](https://crates.io/crates/browsercli)
[![npm](https://img.shields.io/npm/v/@justinhuangcode/browsercli?style=flat-square)](https://www.npmjs.com/package/@justinhuangcode/browsercli)
[![PyPI](https://img.shields.io/pypi/v/browsercli?style=flat-square)](https://pypi.org/project/browsercli/)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Node.js](https://img.shields.io/badge/node.js-18%2B-green.svg?style=flat-square&logo=node.js&logoColor=white)](clients/node/)
[![Python](https://img.shields.io/badge/python-3.9%2B-blue.svg?style=flat-square&logo=python&logoColor=white)](clients/python/)
[![GitHub Stars](https://img.shields.io/github/stars/justinhuangcode/browsercli?style=flat-square&logo=github)](https://github.com/justinhuangcode/browsercli/stargazers)
[![Last Commit](https://img.shields.io/github/last-commit/justinhuangcode/browsercli?style=flat-square)](https://github.com/justinhuangcode/browsercli/commits/main)
[![Issues](https://img.shields.io/github/issues/justinhuangcode/browsercli?style=flat-square)](https://github.com/justinhuangcode/browsercli/issues)
[![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](https://github.com/justinhuangcode/browsercli)

本地优先的浏览器渲染命令行工具，面向 AI 智能体和开发者。在本地目录中编写 HTML/CSS/JS，通过真实的 Chromium 浏览器渲染，完整支持 DevTools 控制。

## 为什么选择 browsercli？

AI Agent 生成 HTML/CSS/JS 之后，需要**看到自己构建的页面**。它们需要在真实浏览器中渲染页面、截图、读取 DOM、检查控制台错误，然后迭代修改——而这一切不需要人类手动操作。

现有工具无法满足这个工作流：

| | browsercli | Playwright | Puppeteer | live-server |
|---|---|---|---|---|
| 为 AI Agent 设计 | 是 | 否（测试框架） | 否（库） | 否（开发工具） |
| 持久化守护进程 | 是 — `start` 一次，随时控制 | 否 — 每次脚本启动新浏览器 | 否 — 每次脚本启动新浏览器 | 无守护进程 |
| 本地文件服务 + 自动刷新 | 内置（250ms） | 无 | 无 | 有，但无自动化能力 |
| CLI + 客户端库 | CLI + Python + Node.js | Python/Node.js/Java/.NET | 仅 Node.js | 无 |
| DOM / 截图 / 控制台 / 网络 | 全部通过 CLI 或 SDK | 仅通过代码 | 仅通过代码 | 无 |
| 插件系统 | 模板 + RPC + 钩子 | 无 | 无 | 无 |
| 上手复杂度 | `browsercli start` | 安装 + 编写测试脚本 | 安装 + 编写脚本 | `npx live-server` |

**AI Agent 使用 browsercli 的典型工作流：**

```
Agent 将 HTML/CSS/JS 写入磁盘
        ↓
browsercli 自动刷新浏览器（250ms）
        ↓
Agent 执行：browsercli screenshot --out page.png
        ↓
Agent 检查截图 / 查询 DOM / 检查控制台
        ↓
Agent 迭代修改代码
```

无需测试框架样板代码。无需管理浏览器生命周期。只需一个持久运行的浏览器，实时反映你的文件并响应命令。

## 特性

- **守护进程架构** -- `browsercli start` 启动后台进程，CLI 命令通过 Unix socket RPC（macOS/Linux）或 TCP localhost（Windows）与其通信
- **静态文件服务** -- 通过 HTTP 提供本地目录服务，自动解析 `index.html`
- **自动刷新** -- 文件监听器以 250ms 防抖触发浏览器刷新
- **App 模式** -- 默认打开无边框 (`--app`) 浏览器窗口
- **隐身模式** -- 尽力降低自动化检测（移除 webdriver 标识）
- **完整 DOM 控制** -- 通过 CSS 选择器进行 query、query-all、attr、click、type、wait 操作
- **JavaScript 执行** -- 在受控标签页中执行任意 JS
- **截图捕获** -- 全页面或指定元素截图（PNG）
- **控制台捕获** -- 查看浏览器控制台输出（log、warn、error、info），支持级别过滤和 `--clear` 清空缓冲区
- **网络日志** -- 检查 HTTP 请求/响应的方法、状态码、资源类型、MIME 类型、大小和耗时；支持 `--clear`
- **性能指标** -- 基于 Navigation Timing Level 2（含旧版 API 回退）获取 DOMContentLoaded 和 Load 事件耗时
- **JSON 输出** -- 所有命令支持 `--json` 参数输出机器可读格式
- **插件系统** -- 通过基于脚本的插件（JSON 清单）扩展 browsercli，支持自定义页面模板、RPC 端点和生命周期钩子
- **跨平台** -- 支持 macOS（App Bundle）、Linux 和 Windows 的 Chrome/Chromium/Edge 自动检测

## 安装

### 预编译二进制（推荐）

从 [GitHub Releases](https://github.com/justinhuangcode/browsercli/releases) 下载适合你平台的二进制文件：

| 平台 | 文件 |
| --- | --- |
| Linux x86_64 | `browsercli-v*-linux-x86_64.tar.gz` |
| Linux ARM64 | `browsercli-v*-linux-arm64.tar.gz` |
| macOS Intel | `browsercli-v*-macos-x86_64.tar.gz` |
| macOS Apple Silicon | `browsercli-v*-macos-arm64.tar.gz` |
| Windows x86_64 | `browsercli-v*-windows-x86_64.zip` |

解压后将二进制文件放入 `$PATH` 即可。

### Homebrew (macOS / Linux)

```bash
brew tap justinhuangcode/tap
brew install browsercli
```

### 通过 Cargo 安装 (crates.io)

```bash
cargo install browsercli
```

### 客户端库

```bash
# Node.js
npm install @justinhuangcode/browsercli

# Python
pip install browsercli
```

### 从源码安装

```bash
cargo install --path .
```

**前置要求：** Rust 1.75+ 和基于 Chromium 的浏览器（Chrome、Chromium、Brave 或 Edge）。在 Windows 上，Microsoft Edge 开箱即用。

## 快速开始

```bash
# 指定项目目录启动
browsercli start --dir ./my-site

# 使用临时目录启动
browsercli start

# 查看状态
browsercli status

# 导航到路径
browsercli goto /

# 查询 DOM
browsercli dom query "h1" --mode text
browsercli dom all "a" --mode outer_html

# 执行 JavaScript
browsercli eval "document.title"

# 截图
browsercli screenshot --out page.png

# 查看控制台输出（--clear 读取后清空缓冲区）
browsercli console --level error
browsercli console --clear

# 查看网络请求（--clear 读取后清空缓冲区）
browsercli network --limit 20
browsercli network --clear

# 性能指标
browsercli perf

# 停止
browsercli stop
```

## 命令列表

| 命令 | 说明 |
| --- | --- |
| `start` | 后台启动守护进程 |
| `serve` | 前台运行（非守护进程模式） |
| `status` | 显示当前会话状态 |
| `stop` | 停止守护进程 |
| `focus` | 将浏览器窗口置于前台（仅 macOS） |
| `devtools` | 打印 DevTools WebSocket URL |
| `goto <path>` | 导航到路径或 URL |
| `eval <expr>` | 执行 JavaScript 表达式 |
| `reload` | 刷新浏览器标签页 |
| `dom` | DOM 工具：query、all、attr、click、type、wait |
| `screenshot` | 捕获页面或元素截图 |
| `console` | 查看浏览器控制台条目 |
| `network` | 查看网络请求日志 |
| `perf` | 显示页面性能指标 |
| `plugin list` | 列出已安装的插件 |
| `plugin init <name>` | 创建插件脚手架 |

## 启动参数

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--dir <path>` | 临时目录 | 服务目录 |
| `--port <n>` | 0（随机） | HTTP 端口 |
| `--devtools-port <n>` | 0（随机） | Chrome DevTools 端口 |
| `--headless` | false | 无头模式运行 |
| `--no-app` | false | 禁用无边框窗口 |
| `--no-stealth` | false | 禁用隐身模式 |
| `--window-size <w,h>` | 1280,720 | 浏览器窗口大小 |
| `--browser-bin <path>` | 自动检测 | Chromium/Chrome 可执行文件路径 |
| `--restart` | false | 重启已运行的实例 |
| `--template <name>` | *（无）* | 启动时应用插件模板 |

## 控制台和网络参数

| 参数 | 适用命令 | 说明 |
| --- | --- | --- |
| `--level <level>` | `console` | 按级别过滤：log、warn、error、info |
| `--limit <n>` | `console`、`network` | 限制返回条目数 |
| `--clear` | `console`、`network` | 读取后清空缓冲区 |

## DOM 子命令

```bash
browsercli dom query "selector" [--mode outer_html|text]
browsercli dom all "selector" [--mode outer_html|text] [--limit N]
browsercli dom attr "selector" "attribute-name"
browsercli dom click "selector"
browsercli dom type "selector" "text" [--clear]
browsercli dom wait "selector" [--state visible|hidden|present|gone] [--timeout 10s]
```

简写形式：

```bash
browsercli dom "#app" --mode text
```

## 插件系统

browsercli 内置插件系统，提供**三大扩展点**：页面模板、自定义 RPC 端点和生命周期钩子。插件是包含 `plugin.json` 清单和可执行脚本的普通目录——无需编译、WASM 或动态链接库。

```
~/.browsercli/plugins/my-plugin/
├── plugin.json              # 清单文件（必需）
├── templates/
│   └── dashboard/           # HTML/CSS/JS 脚手架
│       ├── index.html
│       ├── style.css
│       └── app.js
├── handlers/
│   └── refresh.sh           # 自定义 RPC 端点脚本
└── hooks/
    └── on_start.sh          # 生命周期钩子脚本
```

### 内置模板

browsercli 自带 4 个内置模板，开箱即用，无需安装插件：

| 模板 | 技术栈 | 用途 |
|------|--------|------|
| `tailwind` | Tailwind CSS v4 CDN | 通用响应式 UI |
| `dashboard` | Tailwind CSS v4 + DaisyUI v5 | 管理面板、监控仪表盘 |
| `chart` | Tailwind CSS v4 + Chart.js v4 | 数据可视化（柱状图、折线图、饼图、雷达图） |
| `form` | Tailwind CSS v4 + Alpine.js v3 | 交互式表单（含客户端验证） |

```bash
browsercli start --template tailwind
browsercli start --template dashboard
browsercli start --template chart
browsercli start --template form
```

所有模板都是单文件 HTML + CDN 引入，零构建步骤，即时刷新。

### 1. 页面模板（插件）

插件可以提供额外的模板。模板是 HTML/CSS/JS 脚手架，在启动时复制到服务目录：

```bash
browsercli start --template my-custom-template
```

### 2. 自定义 RPC 端点

插件可在 `/x/` 命名空间下暴露 HTTP 端点。处理脚本从 stdin 接收 JSON，向 stdout 输出 JSON：

```bash
# handlers/refresh.sh
#!/bin/sh
INPUT=$(cat)
echo '{"ok": true, "refreshed_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}'
```

通过客户端库调用：

```typescript
// Node.js
const result = await ac.pluginRpc("/x/dashboard/refresh", { key: "value" });
```

```python
# Python
result = ac.plugin_rpc("/x/dashboard/refresh", {"key": "value"})
```

### 3. 生命周期钩子

由守护进程事件触发的即发即忘脚本：

| 事件 | 触发时机 | 额外上下文 |
| --- | --- | --- |
| `on_daemon_start` | 守护进程就绪 | -- |
| `on_daemon_stop` | 守护进程关闭中 | -- |
| `on_file_change` | 服务目录文件变更 | `$BROWSERCLI_FILE_PATH` |
| `on_navigate` | 浏览器导航 | `$BROWSERCLI_URL` |
| `on_console` | 控制台消息 | stdin 传入 JSON |
| `on_network` | 网络请求 | stdin 传入 JSON |

### 插件 CLI

```bash
browsercli plugin init my-plugin   # 创建插件脚手架
browsercli plugin list             # 列出已安装插件
browsercli start --template name   # 启动时应用插件模板
```

所有脚本均可获取环境变量：`BROWSERCLI_TOKEN`、`BROWSERCLI_HTTP_PORT`、`BROWSERCLI_DIR`、`BROWSERCLI_BASE_URL`、`BROWSERCLI_STATE_DIR`、`BROWSERCLI_PLUGIN_NAME`。

完整开发指南、清单 Schema、安全模型和跨平台说明请参阅 [`PLUGINS.md`](PLUGINS.md)。附带完整的[示例插件](examples/plugins/dashboard/)。

## 工作原理

1. `browsercli start` 启动一个守护进程：
   - 在随机端口启动 HTTP 静态文件服务器
   - 通过 CDP（Chrome DevTools Protocol）启动 Chromium 浏览器
   - 打开 Unix socket（macOS/Linux）或 TCP localhost（Windows）RPC 服务器用于 CLI 通信
   - 监听服务目录的文件变更
   - 将会话状态写入 `~/.browsercli/session.json`（macOS/Linux）或 `%LOCALAPPDATA%\browsercli\session.json`（Windows）

2. 后续 CLI 命令（`goto`、`eval`、`dom` 等）连接到 RPC 端点，向守护进程发送 JSON 请求。

3. 守护进程将 RPC 请求转换为通过 WebSocket 发送的 CDP 命令。

## 架构

```
                             Unix Socket (macOS/Linux)
+-----------+           or TCP localhost (Windows)      +----------+
|  CLI cmd  | --------------> +--------------+ CDP/WS   | Chromium |
|           | <-------------- |    Daemon    | -------> |          |
+-----------+   JSON RPC      |              | <------- +----------+
                              | HTTP Server  |
                              | File Watch   |
                              +--------------+
                                     |
                                     v
                                Local Files
```

## 客户端库

### Node.js

`clients/node/` 提供零依赖的 Node.js 客户端库（TypeScript 编写，附带类型定义）。安装方式：

```bash
cd clients/node && npm install
```

```typescript
import { BrowserCLI } from "@justinhuangcode/browsercli";

const ac = BrowserCLI.connect();   // 读取 ~/.browsercli/session.json
await ac.goto("/");
const title = await ac.domQuery("h1", "text");
await ac.screenshot("", "page.png");
await ac.stop();

// 插件支持
const plugins = await ac.pluginList();
const result = await ac.pluginRpc("/x/my-plugin/action", { key: "value" });
```

完整 API 文档参见 [`clients/node/README.md`](clients/node/README.md)。

### Python

`clients/python/` 提供零依赖的 Python 客户端库。安装方式：

```bash
pip install -e clients/python
```

```python
from browsercli import BrowserCLI

ac = BrowserCLI.connect()   # 读取 ~/.browsercli/session.json
ac.goto("/")
title = ac.dom_query("h1", mode="text")
ac.screenshot(out="page.png")
ac.stop()

# 插件支持
plugins = ac.plugin_list()
result = ac.plugin_rpc("/x/my-plugin/action", {"key": "value"})
```

完整 API 文档参见 [`clients/python/README.md`](clients/python/README.md)。

## 示例

`examples/` 目录提供 Node.js 和 Python 两种语言的端到端示例：

| 脚本 | 说明 |
| --- | --- |
| [`01_write_reload_screenshot.mjs`](examples/01_write_reload_screenshot.mjs) | Agent 写入 HTML，自动刷新后截图 |
| [`02_form_fill_and_submit.mjs`](examples/02_form_fill_and_submit.mjs) | 自动填写表单、点击提交、检查网络日志、导出结果 |
| [`03_debug_report.mjs`](examples/03_debug_report.mjs) | 收集控制台、网络和性能数据，生成 JSON 调试报告 |
| [`01_write_reload_screenshot.py`](examples/01_write_reload_screenshot.py) | 同上，Python 版本 |
| [`02_form_fill_and_submit.py`](examples/02_form_fill_and_submit.py) | 同上，Python 版本 |
| [`03_debug_report.py`](examples/03_debug_report.py) | 同上，Python 版本 |

启动守护进程后运行任意示例：

```bash
browsercli start --dir /tmp/demo-site

# Node.js（需要先构建客户端）
cd clients/node && npm run build && cd ../..
node examples/01_write_reload_screenshot.mjs

# Python
python examples/01_write_reload_screenshot.py

browsercli stop
```

## 项目结构

```
src/
├── main.rs              # CLI 入口与命令分发
├── cli/mod.rs           # 命令行参数定义 (clap)
├── daemon/
│   ├── mod.rs           # 守护进程模块导出
│   └── server.rs        # 守护进程、RPC 处理器、会话管理
├── browser/
│   ├── mod.rs           # 浏览器模块导出
│   ├── controller.rs    # CDP 通信、浏览器生命周期
│   ├── devtools.rs      # DevTools HTTP API 客户端
│   └── find.rs          # Chromium/Chrome 可执行文件自动检测
├── web/
│   ├── mod.rs           # Web 模块导出
│   ├── server.rs        # 静态文件处理器（含 index 解析）
│   └── welcome.rs       # 欢迎页 HTML 模板
├── rpc/
│   ├── mod.rs           # RPC 模块导出
│   ├── types.rs         # 请求/响应类型定义
│   └── client.rs        # RPC 客户端（Unix socket / TCP）
├── plugins/
│   ├── mod.rs           # 插件清单类型、验证、发现
│   ├── registry.rs      # 插件注册表（O(1) 查找）
│   ├── executor.rs      # 脚本执行引擎
│   ├── hooks.rs         # 生命周期钩子分发
│   └── templates.rs     # 模板复制逻辑
└── watch/
    └── mod.rs           # 文件系统监听器（含防抖）
clients/node/                # Node.js 客户端库（TypeScript，零依赖）
clients/python/              # Python 客户端库（零依赖）
examples/                    # 端到端示例脚本
examples/plugins/              # 示例插件（dashboard）
tests/
├── cli_integration.rs       # CLI 集成测试
└── e2e_integration.rs       # 完整生命周期 E2E 测试（需要 Chromium）
```

## 安全性与威胁模型

browsercli 专为**单用户本地开发机**设计。以下是各层安全控制：

| 层级 | 控制措施 | 详情 |
| --- | --- | --- |
| **HTTP 服务器** | 仅绑定本地回环地址 | 绑定 `127.0.0.1`，不对外暴露 |
| **RPC 传输** | Unix socket（macOS/Linux）或 TCP localhost（Windows）+ Bearer token | Socket 位于 `~/.browsercli/sock`，权限 `0600`（Unix）；TCP `127.0.0.1` 绑定随机端口（Windows）；每次请求需携带随机令牌 |
| **会话文件** | 仅所有者可读 | `~/.browsercli/session.json`（Unix）或 `%LOCALAPPDATA%\browsercli\session.json`（Windows）以 `0600` 权限创建（Unix），其他用户无法读取令牌 |
| **静态文件** | 路径穿越防护 | 请求路径经规范化处理后与服务根目录进行校验 |
| **浏览器** | 用户数据隔离 | 每次会话使用独立的 `--user-data-dir` 临时目录 |

### 不适用场景

- **多用户 / 共享服务器** — 具有 root 权限或相同 UID 的本地用户可以读取会话令牌并发起 RPC 命令。若在共享服务器上运行，请通过操作系统权限或容器隔离 `~/.browsercli/` 目录。
- **服务不受信任的内容** — HTTP 服务器仅用于服务你或 AI Agent 编写的本地文件，请勿将 `--dir` 指向不受信任的目录。
- **生产环境** — browsercli 是开发/测试工具，未实现 TLS、速率限制或审计日志。

### 隐身模式

默认情况下，browsercli 会移除 `navigator.webdriver` 标识并应用少量自动化指纹缓解措施，使本地页面的行为与正常浏览器一致（例如，部分前端框架在检测到无头/自动化 Chrome 时会改变行为）。

| 参数 | 行为 |
| --- | --- |
| *（默认）* | 应用隐身补丁 — `navigator.webdriver` 返回 `false` |
| `--no-stealth` | 禁用所有隐身补丁 — 浏览器报告为自动化环境 |

**适用范围：** 隐身模式仅用于自动化检测干扰页面行为的本地开发和测试场景，*不用于*绕过外部网站的安全控制。

## 常见问题

详见 [TROUBLESHOOTING.md](TROUBLESHOOTING.md)（英文），常见问题包括：

- 找不到浏览器 — 各平台安装指引
- 端口冲突
- 无头模式（服务器 / CI 环境）
- 权限错误
- 模板未找到

## 参与贡献

欢迎贡献代码！请参阅 [CONTRIBUTING.md](CONTRIBUTING.md) 了解详细指南。

## 更新日志

查看 [CHANGELOG.md](CHANGELOG.md) 了解版本历史。

## 致谢

本项目受 [steipete/canvas](https://github.com/steipete/canvas) 启发。

## 许可证

[MIT](LICENSE)
