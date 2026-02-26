# browsercli Node.js Client

Zero-dependency Node.js client for the [browsercli](https://github.com/justinhuangcode/browsercli) browser workspace daemon. Written in TypeScript with full type definitions included.

## Installation

```bash
cd clients/node && npm install   # from the repo root
```

## Platform Support

The client auto-detects the RPC transport from the session file:

- **macOS / Linux** — connects via Unix socket (`socket_path` in session.json)
- **Windows** — connects via TCP localhost (`rpc_port` in session.json)

No code changes are needed — `BrowserCLI.connect()` handles both transports.

## Quick Start

```typescript
import { BrowserCLI } from "@justinhuangcode/browsercli";

// Connect to a running daemon
// macOS/Linux: reads ~/.browsercli/session.json
// Windows: reads %LOCALAPPDATA%\browsercli\session.json
const ac = BrowserCLI.connect();

// Navigate and inspect
await ac.goto("/");
const title = await ac.domQuery("h1", "text");
console.log(`Title: ${title}`);

// Evaluate JavaScript
const result = await ac.eval("1 + 1");
console.log(`Result: ${result}`);

// Screenshot
await ac.screenshot("", "page.png");

// Stop the daemon
await ac.stop();
```

## Error Handling

All exceptions inherit from `BrowserCLIError`, so you can catch the whole family or handle specific cases:

```typescript
import {
  BrowserCLI,
  BrowserCLIError,
  ConnectionError,
  AuthenticationError,
  SessionError,
  NotFoundError,
  ServerError,
} from "@justinhuangcode/browsercli";

try {
  const ac = BrowserCLI.connect();
  await ac.domQuery("#missing-element", "text");
} catch (err) {
  if (err instanceof SessionError) {
    console.log("Daemon not running — start it with: browsercli start");
  } else if (err instanceof ConnectionError) {
    console.log("Cannot reach daemon — is the socket file valid?");
  } else if (err instanceof AuthenticationError) {
    console.log("Token rejected — daemon may have restarted, reconnect");
  } else if (err instanceof NotFoundError) {
    console.log(`Element or endpoint not found: ${err.errorMessage}`);
  } else if (err instanceof ServerError) {
    console.log(`Daemon internal error (HTTP ${err.statusCode}): ${err.errorMessage}`);
  } else if (err instanceof BrowserCLIError) {
    console.log(`Unexpected client error: ${err.message}`);
  }
}
```

### Exception Hierarchy

```
BrowserCLIError            # Base — catch-all
├── SessionError            # session.json missing/invalid
├── ConnectionError         # RPC endpoint unreachable (socket or TCP)
├── AuthenticationError     # HTTP 401 (bad token)
└── RPCError                # Any HTTP 4xx/5xx with statusCode + errorMessage
    ├── BadRequestError     # HTTP 400
    ├── NotFoundError       # HTTP 404
    └── ServerError         # HTTP 5xx
```

## Constructor

```typescript
new BrowserCLI(addr: string, token: string, timeout?: number)
```

| Parameter | Type | Description |
| --- | --- | --- |
| `addr` | `string` | Unix socket path (macOS/Linux) or TCP `host:port` (Windows) |
| `token` | `string` | Bearer token for daemon authentication |
| `timeout` | `number` | Request timeout in ms (default `30000`) |

In most cases, use the `connect()` factory instead of calling the constructor directly.

## API Reference

| Method | Description |
| --- | --- |
| `BrowserCLI.connect(sessionPath?, timeout?)` | Create client from session file (auto-detects Unix socket or TCP) |
| `status()` | Daemon and browser status |
| `version()` | RPC and schema version info |
| `goto(url)` | Navigate to a path or URL |
| `eval(expression)` | Evaluate JavaScript |
| `reload()` | Reload the page |
| `domQuery(selector, mode)` | Query a single DOM element |
| `domAll(selector, mode)` | Query all matching elements |
| `domAttr(selector, name)` | Get an element attribute |
| `domClick(selector)` | Click an element |
| `domType(selector, text, clear)` | Type text into an input |
| `domWait(selector, state, timeoutMs)` | Wait for element state |
| `screenshot(selector, out)` | Capture screenshot (PNG Buffer) |
| `console(level, limit, clear)` | Fetch console entries |
| `network(limit, clear)` | Fetch network log entries |
| `perf()` | Page performance metrics |
| `stop()` | Stop the daemon |
| `pluginList()` | List installed plugins |
| `pluginRpc(path, body?)` | Call a custom plugin RPC endpoint (`/x/...`) |

All methods are async and return Promises.

### Valid Parameter Values

| Parameter | Valid Values | Default |
| --- | --- | --- |
| `domQuery` / `domAll` `mode` | `"outer_html"`, `"text"` | `"outer_html"` |
| `domWait` `state` | `"visible"`, `"hidden"`, `"attached"`, `"detached"` | `"visible"` |
| `console` `level` | `""` (all), `"log"`, `"warn"`, `"error"`, `"info"` | `""` |

Invalid values throw `TypeError` before any RPC call is made.

## Running Tests

```bash
cd clients/node
npm test
```

Tests include:
- **Session parsing**: valid/invalid/missing session files (including `rpc_port` for Windows)
- **Constructor validation**: invalid addr, token, timeout
- **Parameter validation**: client-side checks for all method arguments
- **Contract tests**: mock HTTP server (Unix socket on macOS/Linux, TCP on Windows) emulating the Rust daemon
- **Error handling**: auth failures, 400/404/500 errors, connection errors
- **Exception hierarchy**: inheritance and attribute verification

## Requirements

- Node.js 18+
- A running `browsercli` daemon (`browsercli start`)
- No external dependencies (Node.js stdlib only)
