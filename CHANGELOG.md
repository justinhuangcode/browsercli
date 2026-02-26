# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.0.1] - 2026-02-26

### Added

- **Homebrew tap** -- `brew tap justinhuangcode/tap && brew install browsercli` with auto-updated formula on release
- **Full multi-channel publishing** -- crates.io, npm, PyPI, and Homebrew tap all configured and enabled

## [1.0.0] - 2026-02-26

### Changed

- **First stable release** -- API is now considered stable and will follow semantic versioning
- Version bump across all packages (Rust CLI, Node.js client, Python client)
- Python package status upgraded from Alpha to Production/Stable
- Added package metadata for npm and PyPI distribution
- Optimized crate with `exclude` patterns for smaller crates.io package

### Added

- **Release workflow** -- automated cross-platform binary builds via GitHub Actions on tag push
- **Multi-channel publishing** -- crates.io, npm, and PyPI with graceful fallback when tokens not configured
- **SHA256 checksums** for all release binaries
- **Linux ARM64 builds** -- `aarch64-unknown-linux-gnu` added to CI and release matrix
- **macOS Intel builds** -- `x86_64-apple-darwin` added to release matrix (5 targets total)

## [0.4.0] - 2025-02-26

### Added

- **Plugin system**: script-based plugin architecture with JSON manifests supporting custom page templates, RPC endpoints, and lifecycle hooks
- **Plugin manifest validation**: name, version, script path, and RPC path validation with security checks (path traversal prevention)
- **Plugin discovery**: automatic scanning of `~/.browsercli/plugins/` (macOS/Linux) or `%LOCALAPPDATA%\browsercli\plugins\` (Windows)
- **Plugin registry**: central O(1) lookup for RPC handlers, hook handlers, and templates with conflict warnings
- **Script executor**: async script execution with configurable timeout, cross-platform interpreter detection, and stdin/stdout/stderr piping
- **Lifecycle hooks**: fire-and-forget hook dispatch for `on_daemon_start`, `on_daemon_stop`, `on_file_change`, `on_navigate`, `on_console`, `on_network`
- **Template system**: `--template <name>` flag on `start` and `serve` commands to apply plugin templates at startup
- **Custom RPC endpoints**: `/x/<plugin>/<action>` namespace for plugin-defined HTTP endpoints with JSON protocol
- **`/plugins` RPC endpoint**: list all installed plugins with their templates, hooks, and RPC endpoints
- **`plugin list` CLI command**: display installed plugins, templates, hooks, and endpoints
- **`plugin init <name>` CLI command**: scaffold a new plugin directory with manifest, example template, hook, and RPC handler
- **Node.js client `pluginList()`**: fetch installed plugins from the daemon
- **Node.js client `pluginRpc(path, body?)`**: call custom plugin RPC endpoints with `/x/` prefix validation
- **Python client `plugin_list()`**: fetch installed plugins from the daemon
- **Python client `plugin_rpc(path, body=None)`**: call custom plugin RPC endpoints with `/x/` prefix validation
- **Node.js plugin tests**: 8 new tests (parameter validation, contract, TCP transport)
- **Python plugin tests**: 8 new tests (parameter validation, contract, TCP transport)
- **Example plugin**: `examples/plugins/dashboard/` with template, RPC handlers, and lifecycle hooks
- **`PLUGINS.md`**: comprehensive plugin development guide

## [0.3.0] - 2025-02-26

### Added

- **Windows platform support**: full daemon, CLI, and client library support for Windows
- **Windows RPC transport**: TCP localhost with Bearer token authentication (Unix socket behavior unchanged on macOS/Linux)
- **Windows browser detection**: auto-detect Chrome, Chromium, Brave, and Edge in Program Files, Program Files (x86), and LocalAppData
- **Windows state directory**: session data stored in `%LOCALAPPDATA%\browsercli\` instead of `~/.browsercli/`
- **Windows process management**: `CREATE_NEW_PROCESS_GROUP` for daemon and browser isolation, `OpenProcess` for alive detection via `windows-sys` crate
- **Node.js client TCP transport**: automatic Unix socket or TCP connection based on session file contents; Windows session path support
- **Python client TCP transport**: automatic Unix socket or TCP connection based on session file contents; Windows session path support
- **Node.js client TCP contract tests**: 6 additional tests verifying all RPC operations over TCP transport
- **Python client TCP contract tests**: 7 additional tests verifying all RPC operations over TCP transport
- **Windows CI**: `windows-latest` added to Rust test, Node.js client, Python client, and release build matrices
- **Windows release build**: `x86_64-pc-windows-msvc` target added to CI build matrix

## [0.2.0] - 2025-02-26

### Added

- **Schema versioning**: all `--json` output includes `schema_version` field for forward-compatible parsing
- **RPC versioning**: `/version` endpoint returns `rpc_version` and `schema_version`; compatibility policy documented in `rpc/types.rs`
- **Structured error output**: `--json` mode emits `{"schema_version": 1, "error": "..."}` on failure instead of unstructured text
- **Predictable exit codes**: exit 0 on success, exit 1 on any error
- **Logs to stderr**: tracing/log output goes to stderr so stdout remains clean for machine-readable data
- **E2E integration test**: full lifecycle test (start → goto → dom → eval → screenshot → console → network → perf → stop) gated behind `BROWSERCLI_E2E=1`
- **CI matrix**: tests now run on both `ubuntu-latest` and `macos-latest`
- **Browser alive detection**: `status` command reports whether the browser process is still alive
- **Security & Threat Model**: README documents localhost-only binding, token storage, session permissions, and multi-user warnings
- **Stealth mode documentation**: expanded docs with behavior table for default vs `--no-stealth`
- **Node.js client library** (`clients/node/`): zero-dependency client written in TypeScript with full type definitions — all 16 RPC endpoints (status, version, goto, eval, reload, domQuery, domAll, domAttr, domClick, domType, domWait, screenshot, console, network, perf, stop)
- **Node.js client error hierarchy**: `BrowserCLIError` base with `SessionError`, `ConnectionError`, `AuthenticationError`, `RPCError` (`BadRequestError`, `NotFoundError`, `ServerError`) — matches Rust daemon HTTP status codes
- **Node.js client parameter validation**: client-side validation for all method arguments with TypeScript type safety and runtime checks
- **Node.js client contract tests**: 69 tests with mock Unix socket HTTP server covering session parsing, constructor validation, parameter validation, all 16 RPC endpoints, error handling, and exception hierarchy
- **Node.js client CI**: test job in GitHub Actions matrix across Node.js 18 and 22
- **Python client library** (`clients/python/`): zero-dependency client with full API coverage (status, goto, eval, dom, screenshot, console, network, perf, stop)
- **Python client exception hierarchy**: `BrowserCLIError` base with `SessionError`, `ConnectionError`, `AuthenticationError`, `RPCError` (`BadRequestError`, `NotFoundError`, `ServerError`) — matches Rust daemon HTTP status codes
- **Python client parameter validation**: client-side validation for all method arguments (`mode`, `state`, `level`, selectors, timeouts) with clear error messages
- **Python client logging**: `logging` module integration under `browsercli` logger name
- **Python client contract tests**: 64 tests with mock Unix socket server emulating the Rust daemon, covering all 16 RPC endpoints, auth failures, error codes, parameter validation, and exception hierarchy
- **Python client CI**: test job in GitHub Actions matrix across Python 3.9 and 3.12
- **End-to-end examples** (`examples/`): Node.js and Python scripts demonstrating agent write+reload+screenshot, form fill+submit+network export, and console/network/perf debug report

### Fixed

- **CI E2E reliability**: connect to page-level WebSocket for direct CDP commands, fix missing port in DevTools WS URL, add 10s CDP command timeout, increase daemon startup timeout to 30s
- **`CHROME_BIN` env var**: override browser binary path for CI and non-standard installs
- **Headless sandbox flags**: `--no-sandbox` and `--disable-dev-shm-usage` for containerized/CI environments
- **Daemon startup diagnostics**: stderr redirected to `~/.browsercli/daemon.log` with contents included in timeout error messages
- **Network logging**: `method` field now contains the real HTTP method (GET/POST/...) instead of CDP resource type; resource type is available as a separate `resource_type` field
- **Network logging**: `duration_ms` is computed from actual request/response timing instead of hardcoded 0
- **Focus command**: reads `session.json` to detect the actual browser (Chrome, Chromium, Brave, Edge) instead of hardcoding "Google Chrome"
- **Performance metrics**: upgraded to Navigation Timing Level 2 (`performance.getEntriesByType('navigation')`) with legacy `performance.timing` fallback
- **Session security**: `session.json` is now written with `0600` permissions before rename-into-place

## [0.1.1] - 2025-02-26

### Added

- `--clear` flag for `console` and `network` commands to drain the buffer after reading
- Console & Network Flags documentation section in README

## [0.1.0] - 2025-02-26

### Added

- Daemon architecture with background process and Unix socket RPC
- Static file server with automatic `index.html` resolution
- Auto-reload via file watcher with 250ms debounce
- App mode (chromeless `--app` window) enabled by default
- Stealth mode for automation detection reduction
- Full DOM control: `query`, `all`, `attr`, `click`, `type`, `wait`
- JavaScript evaluation via `eval` command
- Screenshot capture (full page or element-specific, PNG)
- Console capture with level filtering (`log`, `warn`, `error`, `info`)
- Network request logging with method, status, resource type, MIME type, size, and duration
- Performance metrics (Navigation Timing Level 2 with legacy fallback)
- JSON output mode (`--json`) for all commands
- Cross-platform support: macOS and Linux
- Headless mode support
- GitHub Actions CI (check, test, fmt, clippy, release builds)

[1.0.1]: https://github.com/justinhuangcode/browsercli/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/justinhuangcode/browsercli/compare/v0.4.0...v1.0.0
[0.4.0]: https://github.com/justinhuangcode/browsercli/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/justinhuangcode/browsercli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/justinhuangcode/browsercli/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/justinhuangcode/browsercli/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/justinhuangcode/browsercli/releases/tag/v0.1.0
