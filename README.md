# browsercli

**English** | [中文](./README_CN.md)

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

A browser visual workspace for AI agents. Write HTML/CSS/JS in a local directory and have it rendered in a real Chromium browser with full DevTools control — all from the command line.

## Features

- **Daemon architecture** — `browsercli start` launches a background process; CLI commands talk to it via Unix socket RPC (macOS/Linux) or TCP localhost (Windows)
- **Static file server** — Serves a local directory over HTTP with automatic `index.html` resolution
- **Auto-reload** — File watcher with 250ms debounce triggers browser reload on save
- **App mode** — Opens a chromeless (`--app`) browser window by default
- **Stealth mode** — Best-effort automation detection reduction (webdriver flag removal)
- **Full DOM control** — Query, query-all, attr, click, type, wait via CSS selectors
- **JavaScript evaluation** — Execute arbitrary JS in the controlled tab
- **Screenshot capture** — Full page or element-specific (PNG)
- **Console capture** — View browser console output (log, warn, error, info) with level filtering and `--clear` to drain the buffer
- **Network logging** — Inspect HTTP requests/responses with method, status, resource type, MIME type, size, and duration; supports `--clear`
- **Performance metrics** — Navigation Timing Level 2 (with legacy fallback) for DOMContentLoaded and Load timing
- **JSON output** — Pass `--json` for machine-readable output on every command
- **Plugin system** — Extend browsercli with custom page templates, RPC endpoints, and lifecycle hooks via script-based plugins with JSON manifests
- **Cross-platform** — macOS (app bundles), Linux, and Windows Chrome/Chromium/Edge auto-detection

## Installation

### Pre-built binaries (recommended)

Download the latest binary for your platform from [GitHub Releases](https://github.com/justinhuangcode/browsercli/releases):

| Platform | Archive |
| --- | --- |
| Linux x86_64 | `browsercli-v*-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 | `browsercli-v*-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `browsercli-v*-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `browsercli-v*-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `browsercli-v*-x86_64-pc-windows-msvc.zip` |

Extract the archive and place the binary in your `$PATH`.

### Homebrew (macOS / Linux)

```bash
brew tap justinhuangcode/tap
brew install browsercli
```

### Via Cargo (crates.io)

```bash
cargo install browsercli
```

### Client libraries

```bash
# Node.js
npm install @justinhuangcode/browsercli

# Python
pip install browsercli
```

### From source

```bash
cargo install --path .
```

**Requirements:** Rust 1.75+ and a Chromium-based browser (Chrome, Chromium, Brave, or Edge). On Windows, Microsoft Edge works out of the box.

## Quick Start

```bash
# Start with a project directory
browsercli start --dir ./my-site

# Or start with a temp directory
browsercli start

# Check status
browsercli status

# Navigate to a path
browsercli goto /

# Query the DOM
browsercli dom query "h1" --mode text
browsercli dom all "a" --mode outer_html

# Evaluate JavaScript
browsercli eval "document.title"

# Take a screenshot
browsercli screenshot --out page.png

# View console output (--clear drains the buffer)
browsercli console --level error
browsercli console --clear

# View network requests (--clear drains the buffer)
browsercli network --limit 20
browsercli network --clear

# Performance timing
browsercli perf

# Stop
browsercli stop
```

## Commands

| Command | Description |
| --- | --- |
| `start` | Launch daemon in background |
| `serve` | Run in foreground (no daemon) |
| `status` | Show current session status |
| `stop` | Stop the daemon |
| `focus` | Bring browser window to front (macOS) |
| `devtools` | Print DevTools WebSocket URL |
| `goto <path>` | Navigate to a path or URL |
| `eval <expr>` | Evaluate JavaScript |
| `reload` | Reload the browser tab |
| `dom` | DOM utilities: query, all, attr, click, type, wait |
| `screenshot` | Capture page or element screenshot |
| `console` | View browser console entries |
| `network` | View network request log |
| `perf` | Show page performance metrics |
| `plugin list` | List installed plugins |
| `plugin init <name>` | Scaffold a new plugin |

## Start Flags

| Flag | Default | Description |
| --- | --- | --- |
| `--dir <path>` | temp dir | Directory to serve |
| `--port <n>` | 0 (random) | HTTP port |
| `--devtools-port <n>` | 0 (random) | Chrome DevTools port |
| `--headless` | false | Run browser headless |
| `--no-app` | false | Disable chromeless window |
| `--no-stealth` | false | Disable stealth mode |
| `--window-size <w,h>` | 1280,720 | Browser window size |
| `--browser-bin <path>` | auto-detect | Chromium/Chrome binary path |
| `--restart` | false | Restart if already running |
| `--template <name>` | *(none)* | Apply a plugin template at startup |

## Console & Network Flags

| Flag | Applies To | Description |
| --- | --- | --- |
| `--level <level>` | `console` | Filter by level: log, warn, error, info |
| `--limit <n>` | `console`, `network` | Limit number of returned entries |
| `--clear` | `console`, `network` | Drain the buffer after reading |

## DOM Subcommands

```bash
browsercli dom query "selector" [--mode outer_html|text]
browsercli dom all "selector" [--mode outer_html|text] [--limit N]
browsercli dom attr "selector" "attribute-name"
browsercli dom click "selector"
browsercli dom type "selector" "text" [--clear]
browsercli dom wait "selector" [--state visible|hidden|present|gone] [--timeout 10s]
```

Shorthand:

```bash
browsercli dom "#app" --mode text
```

## Plugin System

browsercli has a built-in plugin system with **three extension points**: page templates, custom RPC endpoints, and lifecycle hooks. Plugins are plain directories with a `plugin.json` manifest and executable scripts -- no compilation, WASM, or dynamic libraries required.

```
~/.browsercli/plugins/my-plugin/
├── plugin.json              # Manifest (required)
├── templates/
│   └── dashboard/           # HTML/CSS/JS scaffold
│       ├── index.html
│       ├── style.css
│       └── app.js
├── handlers/
│   └── refresh.sh           # Custom RPC endpoint script
└── hooks/
    └── on_start.sh          # Lifecycle hook script
```

### 1. Page Templates

Templates are HTML/CSS/JS scaffolds that get copied to the serve directory at startup:

```bash
browsercli start --template dashboard
```

### 2. Custom RPC Endpoints

Plugins can expose HTTP endpoints under the `/x/` namespace. Handler scripts receive JSON on stdin and write JSON to stdout:

```bash
# handlers/refresh.sh
#!/bin/sh
INPUT=$(cat)
echo '{"ok": true, "refreshed_at": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"}'
```

Call from client libraries:

```typescript
// Node.js
const result = await ac.pluginRpc("/x/dashboard/refresh", { key: "value" });
```

```python
# Python
result = ac.plugin_rpc("/x/dashboard/refresh", {"key": "value"})
```

### 3. Lifecycle Hooks

Fire-and-forget scripts triggered by daemon events:

| Event | Trigger | Extra Context |
| --- | --- | --- |
| `on_daemon_start` | Daemon is ready | -- |
| `on_daemon_stop` | Daemon shutting down | -- |
| `on_file_change` | File changed in serve dir | `$BROWSERCLI_FILE_PATH` |
| `on_navigate` | Browser navigated | `$BROWSERCLI_URL` |
| `on_console` | Console message | JSON on stdin |
| `on_network` | Network request | JSON on stdin |

### Plugin CLI

```bash
browsercli plugin init my-plugin   # Scaffold a new plugin
browsercli plugin list             # List installed plugins
browsercli start --template name   # Apply a plugin template at startup
```

All scripts receive environment variables: `BROWSERCLI_TOKEN`, `BROWSERCLI_HTTP_PORT`, `BROWSERCLI_DIR`, `BROWSERCLI_BASE_URL`, `BROWSERCLI_STATE_DIR`, `BROWSERCLI_PLUGIN_NAME`.

See [`PLUGINS.md`](PLUGINS.md) for the full development guide, manifest schema, security model, and cross-platform notes. A complete [example plugin](examples/plugins/dashboard/) is included.

## How It Works

1. `browsercli start` spawns a daemon process that:
   - Starts an HTTP static file server on a random port
   - Launches a Chromium browser via CDP (Chrome DevTools Protocol)
   - Opens a Unix socket (macOS/Linux) or TCP localhost (Windows) RPC server for CLI communication
   - Watches the served directory for file changes
   - Writes session state to `~/.browsercli/session.json` (macOS/Linux) or `%LOCALAPPDATA%\browsercli\session.json` (Windows)

2. Subsequent CLI commands (`goto`, `eval`, `dom`, etc.) connect to the RPC endpoint and send JSON requests to the daemon.

3. The daemon translates RPC requests into CDP commands over WebSocket.

## Architecture

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

## Client Libraries

### Node.js

A zero-dependency Node.js client (written in TypeScript, ships with type definitions) is included in `clients/node/`. Install it with:

```bash
cd clients/node && npm install
```

```typescript
import { BrowserCLI } from "@justinhuangcode/browsercli";

const ac = BrowserCLI.connect();   // reads ~/.browsercli/session.json
await ac.goto("/");
const title = await ac.domQuery("h1", "text");
await ac.screenshot("", "page.png");
await ac.stop();

// Plugin support
const plugins = await ac.pluginList();
const result = await ac.pluginRpc("/x/my-plugin/action", { key: "value" });
```

See [`clients/node/README.md`](clients/node/README.md) for the full API reference.

### Python

A zero-dependency Python client is included in `clients/python/`. Install it with:

```bash
pip install -e clients/python
```

```python
from browsercli import BrowserCLI

ac = BrowserCLI.connect()   # reads ~/.browsercli/session.json
ac.goto("/")
title = ac.dom_query("h1", mode="text")
ac.screenshot(out="page.png")
ac.stop()

# Plugin support
plugins = ac.plugin_list()
result = ac.plugin_rpc("/x/my-plugin/action", {"key": "value"})
```

See [`clients/python/README.md`](clients/python/README.md) for the full API reference.

## Examples

End-to-end examples are provided in `examples/` for both Node.js and Python:

| Script | Description |
| --- | --- |
| [`01_write_reload_screenshot.mjs`](examples/01_write_reload_screenshot.mjs) | Agent writes HTML, auto-reload picks it up, takes a screenshot |
| [`02_form_fill_and_submit.mjs`](examples/02_form_fill_and_submit.mjs) | Fills a form, clicks submit, inspects network log, exports results |
| [`03_debug_report.mjs`](examples/03_debug_report.mjs) | Collects console, network, and perf data into a JSON debug report |
| [`01_write_reload_screenshot.py`](examples/01_write_reload_screenshot.py) | Same as above, Python version |
| [`02_form_fill_and_submit.py`](examples/02_form_fill_and_submit.py) | Same as above, Python version |
| [`03_debug_report.py`](examples/03_debug_report.py) | Same as above, Python version |

Run any example after starting the daemon:

```bash
browsercli start --dir /tmp/demo-site

# Node.js (build the client first)
cd clients/node && npm run build && cd ../..
node examples/01_write_reload_screenshot.mjs

# Python
python examples/01_write_reload_screenshot.py

browsercli stop
```

## Project Structure

```
src/
├── main.rs              # CLI entry point and command dispatch
├── cli/mod.rs           # Command-line argument definitions (clap)
├── daemon/
│   ├── mod.rs           # Daemon module exports
│   └── server.rs        # Daemon process, RPC handler, session management
├── browser/
│   ├── mod.rs           # Browser module exports
│   ├── controller.rs    # CDP communication, browser lifecycle
│   ├── devtools.rs      # DevTools HTTP API client
│   └── find.rs          # Chromium/Chrome binary auto-detection
├── web/
│   ├── mod.rs           # Web module exports
│   ├── server.rs        # Static file handler with index resolution
│   └── welcome.rs       # Welcome page HTML template
├── rpc/
│   ├── mod.rs           # RPC module exports
│   ├── types.rs         # Request/response type definitions
│   └── client.rs        # RPC client (Unix socket / TCP)
├── plugins/
│   ├── mod.rs           # Plugin manifest types, validation, discovery
│   ├── registry.rs      # Central plugin registry with O(1) lookups
│   ├── executor.rs      # Script execution engine
│   ├── hooks.rs         # Lifecycle hook dispatch
│   └── templates.rs     # Template copy logic
└── watch/
    └── mod.rs           # File system watcher with debounce
clients/node/                # Node.js client library (TypeScript, zero dependencies)
clients/python/              # Python client library (zero dependencies)
examples/                    # End-to-end example scripts
examples/plugins/              # Example plugins (dashboard)
tests/
├── cli_integration.rs       # CLI integration tests
└── e2e_integration.rs       # Full lifecycle E2E test (requires Chromium)
```

## Security & Threat Model

browsercli is designed for **single-user, local-only** use on development machines. The following controls are in place:

| Layer | Control | Detail |
| --- | --- | --- |
| **HTTP server** | Localhost-only binding | Binds to `127.0.0.1`; never exposed to the network |
| **RPC transport** | Unix socket (macOS/Linux) or TCP localhost (Windows) + Bearer token | Socket at `~/.browsercli/sock` with `0600` permissions (Unix); TCP `127.0.0.1` bound to a random port (Windows); every request requires a random token |
| **Session file** | Owner-only permissions | `~/.browsercli/session.json` (Unix) or `%LOCALAPPDATA%\browsercli\session.json` (Windows) is created with mode `0600` (Unix) so other users cannot read the token |
| **Static files** | Path traversal protection | Requested paths are canonicalized and checked against the serve root |
| **Browser** | User data isolation | Each session uses a dedicated `--user-data-dir` in a temp directory |

### Not recommended for

- **Multi-user / shared machines** — Other local users with root or same-UID access can read the session token and issue RPC commands. If you run on a shared server, restrict access to `~/.browsercli/` via OS-level permissions or containers.
- **Serving untrusted content** — The HTTP server is intended for local files authored by you or your agent. Do not point `--dir` at untrusted directories.
- **Production workloads** — browsercli is a development/testing tool. It does not implement TLS, rate limiting, or audit logging.

### Stealth mode

By default, browsercli removes the `navigator.webdriver` flag and applies minor automation-fingerprint mitigations so that local pages behave as they would in a normal browser (e.g., some front-end frameworks alter behavior when they detect headless/automated Chrome).

| Flag | Behavior |
| --- | --- |
| *(default)* | Stealth patches applied — `navigator.webdriver` returns `false` |
| `--no-stealth` | All stealth patches disabled — browser reports as automated |

**Scope:** Stealth mode is strictly for local development and testing where automation detection interferes with page behavior. It is *not* designed for bypassing security controls on external websites.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release history.

## Acknowledgments

Inspired by [steipete/canvas](https://github.com/steipete/canvas).

## License

[MIT](LICENSE)
