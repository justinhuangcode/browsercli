# browsercli Python Client

Zero-dependency Python client for the [browsercli](https://github.com/justinhuangcode/browsercli) browser workspace daemon.

## Installation

```bash
pip install -e clients/python   # from the repo root
```

## Platform Support

The client auto-detects the RPC transport from the session file:

- **macOS / Linux** — connects via Unix socket (`socket_path` in session.json)
- **Windows** — connects via TCP localhost (`rpc_port` in session.json)

No code changes are needed — `BrowserCLI.connect()` handles both transports.

## Quick Start

```python
from browsercli import BrowserCLI

# Connect to a running daemon
# macOS/Linux: reads ~/.browsercli/session.json
# Windows: reads %LOCALAPPDATA%\browsercli\session.json
ac = BrowserCLI.connect()

# Navigate and inspect
ac.goto("/")
title = ac.dom_query("h1", mode="text")
print(f"Title: {title}")

# Evaluate JavaScript
result = ac.eval("1 + 1")
print(f"Result: {result}")

# Screenshot
ac.screenshot(out="page.png")

# Stop the daemon
ac.stop()
```

## Error Handling

All exceptions inherit from `BrowserCLIError`, so you can catch the whole family or handle specific cases:

```python
from browsercli import (
    BrowserCLI,
    BrowserCLIError,
    ConnectionError,
    AuthenticationError,
    SessionError,
    NotFoundError,
    ServerError,
)

try:
    ac = BrowserCLI.connect()
    ac.dom_query("#missing-element", mode="text")
except SessionError:
    print("Daemon not running — start it with: browsercli start")
except ConnectionError:
    print("Cannot reach daemon — is the socket file valid?")
except AuthenticationError:
    print("Token rejected — daemon may have restarted, reconnect")
except NotFoundError as e:
    print(f"Element or endpoint not found: {e}")
except ServerError as e:
    print(f"Daemon internal error (HTTP {e.status_code}): {e.error_message}")
except BrowserCLIError as e:
    print(f"Unexpected client error: {e}")
```

### Exception Hierarchy

```
BrowserCLIError            # Base — catch-all
├── SessionError            # session.json missing/invalid
├── ConnectionError         # RPC endpoint unreachable (socket or TCP)
├── AuthenticationError     # HTTP 401 (bad token)
└── RPCError                # Any HTTP 4xx/5xx with status_code + error_message
    ├── BadRequestError     # HTTP 400
    ├── NotFoundError       # HTTP 404
    └── ServerError         # HTTP 5xx
```

## Constructor

```python
BrowserCLI(socket_path="", token="", timeout=30.0, *, rpc_port=0)
```

| Parameter | Type | Description |
| --- | --- | --- |
| `socket_path` | `str` | Unix socket path (macOS/Linux). Mutually exclusive with `rpc_port`. |
| `token` | `str` | Bearer token for daemon authentication |
| `timeout` | `float` | Request timeout in seconds (default `30.0`) |
| `rpc_port` | `int` | TCP port on localhost (Windows). Keyword-only. |

You must provide either `socket_path` or `rpc_port`. In most cases, use the `connect()` factory instead of calling the constructor directly.

## API Reference

| Method | Description |
| --- | --- |
| `BrowserCLI.connect(session_path=None, timeout=30.0)` | Create client from session file (auto-detects Unix socket or TCP) |
| `status()` | Daemon and browser status |
| `version()` | RPC and schema version info |
| `goto(url)` | Navigate to a path or URL |
| `eval(expression)` | Evaluate JavaScript |
| `reload()` | Reload the page |
| `dom_query(selector, mode)` | Query a single DOM element |
| `dom_all(selector, mode)` | Query all matching elements |
| `dom_attr(selector, name)` | Get an element attribute |
| `dom_click(selector)` | Click an element |
| `dom_type(selector, text, clear)` | Type text into an input |
| `dom_wait(selector, state, timeout_ms)` | Wait for element state |
| `screenshot(selector, out)` | Capture screenshot (PNG bytes) |
| `console(level, limit, clear)` | Fetch console entries |
| `network(limit, clear)` | Fetch network log entries |
| `perf()` | Page performance metrics |
| `stop()` | Stop the daemon |
| `plugin_list()` | List installed plugins |
| `plugin_rpc(rpc_path, body=None)` | Call a custom plugin RPC endpoint (`/x/...`) |

### Valid Parameter Values

| Parameter | Valid Values | Default |
| --- | --- | --- |
| `dom_query` / `dom_all` `mode` | `"outer_html"`, `"text"` | `"outer_html"` |
| `dom_wait` `state` | `"visible"`, `"hidden"`, `"attached"`, `"detached"` | `"visible"` |
| `console` `level` | `""` (all), `"log"`, `"warn"`, `"error"`, `"info"` | `""` |

Invalid values raise `ValueError` before any RPC call is made.

## Logging

The client uses Python's `logging` module under the logger name `browsercli`:

```python
import logging
logging.basicConfig(level=logging.DEBUG)
# Now all RPC requests and responses are logged.
```

## Running Tests

```bash
cd clients/python
python -m unittest tests.test_client -v
```

Tests include:
- **Session parsing**: valid/invalid/missing session files (including `rpc_port` for Windows)
- **Parameter validation**: client-side checks for all method arguments
- **Contract tests**: mock server (Unix socket on macOS/Linux, TCP on Windows) emulating the Rust daemon
- **Error handling**: auth failures, 400/404/500 errors, connection errors
- **Exception hierarchy**: inheritance and attribute verification

## Requirements

- Python 3.9+
- A running `browsercli` daemon (`browsercli start`)
- No external dependencies (stdlib only)
