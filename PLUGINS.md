# Plugin Development Guide

This guide covers how to create, install, and use plugins for browsercli.

## Overview

Plugins extend browsercli with three capabilities:

1. **Templates** — HTML/CSS/JS scaffolds copied to the serve directory at startup
2. **Custom RPC endpoints** — Scripts invoked via HTTP at `/x/<plugin>/<action>` paths
3. **Lifecycle hooks** — Fire-and-forget scripts triggered by daemon events

Plugins are directories containing a `plugin.json` manifest and executable scripts. No compilation, WASM, or dynamic libraries required — just scripts that read JSON from stdin and write JSON to stdout.

## Quick Start

```bash
# Scaffold a new plugin
browsercli plugin init my-plugin

# The scaffold is created at:
#   macOS/Linux: ~/.browsercli/plugins/my-plugin/
#   Windows:     %LOCALAPPDATA%\browsercli\plugins\my-plugin\

# List installed plugins
browsercli plugin list

# Start with a plugin template
browsercli start --template my-template
```

## Plugin Directory Structure

```
~/.browsercli/plugins/my-plugin/
├── plugin.json              # Manifest (required)
├── templates/
│   └── dashboard/           # Template source directory
│       ├── index.html       # Entrypoint
│       ├── style.css
│       └── app.js
├── handlers/
│   └── action.sh            # RPC handler script
└── hooks/
    ├── on_start.sh          # Lifecycle hook
    └── on_navigate.sh
```

## Manifest (`plugin.json`)

Every plugin must have a `plugin.json` file in its root directory.

```json
{
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "A brief description of the plugin",
  "author": "Your Name",
  "templates": {
    "dashboard": {
      "description": "Analytics dashboard template",
      "source": "templates/dashboard/",
      "entrypoint": "index.html"
    }
  },
  "hooks": {
    "on_daemon_start": "hooks/on_start.sh",
    "on_navigate": "hooks/on_navigate.sh"
  },
  "rpc": {
    "endpoints": [
      {
        "path": "/x/my-plugin/refresh",
        "handler": "handlers/refresh.sh",
        "method": "POST",
        "description": "Refresh data"
      }
    ]
  }
}
```

### Manifest Fields

| Field | Required | Description |
| --- | --- | --- |
| `name` | Yes | Plugin name. 1-64 characters, alphanumeric, hyphens, and underscores only. |
| `version` | Yes | Semantic version (`X.Y.Z`). |
| `description` | No | Brief description shown in `plugin list`. |
| `author` | No | Author name or contact. |
| `templates` | No | Map of template name to template entry. |
| `hooks` | No | Map of event name to script path. |
| `rpc` | No | RPC configuration with endpoint definitions. |

### Validation Rules

- **Name**: must match `^[a-zA-Z0-9_-]{1,64}$`
- **Version**: must be `X.Y.Z` where X, Y, Z are non-negative integers
- **Script paths**: must be relative (no leading `/`) and must not contain `..`
- **RPC paths**: must start with `/x/`
- **Template sources**: must be relative paths without `..`

## Templates

Templates are directories of static files (HTML, CSS, JS, images, etc.) that get copied to the serve directory when browsercli starts with `--template <name>`.

### Template Entry

```json
{
  "description": "A dashboard template",
  "source": "templates/dashboard/",
  "entrypoint": "index.html"
}
```

| Field | Default | Description |
| --- | --- | --- |
| `description` | `""` | Shown in `plugin list` output |
| `source` | *(required)* | Relative path to the template directory within the plugin |
| `entrypoint` | `"index.html"` | File that must exist in the source directory |

### Copy Behavior

When `--template dashboard` is passed:

1. The template source directory is located via the plugin registry
2. All files are recursively copied to the serve directory
3. Hidden files (starting with `.`), `node_modules/`, `.git/`, `target/`, and `__pycache__/` are skipped
4. Symbolic links are skipped for security
5. The entrypoint file must exist in the source or the copy fails

### Usage

```bash
# Start with a template
browsercli start --template dashboard

# Start with a template and a specific directory
browsercli start --dir ./my-project --template dashboard
```

## Custom RPC Endpoints

Plugins can define HTTP endpoints under the `/x/` namespace. These are handled by executing a script that receives JSON on stdin and writes JSON to stdout.

### Endpoint Definition

```json
{
  "path": "/x/my-plugin/refresh",
  "handler": "handlers/refresh.sh",
  "method": "POST",
  "description": "Refresh dashboard data"
}
```

| Field | Default | Description |
| --- | --- | --- |
| `path` | *(required)* | HTTP path, must start with `/x/` |
| `handler` | *(required)* | Relative path to the handler script |
| `method` | `"POST"` | HTTP method (currently all endpoints accept POST) |
| `description` | `""` | Shown in `plugin list` output |

### Handler Protocol

The handler script:

1. Receives the HTTP request body as JSON on **stdin** (may be empty)
2. Must write a JSON response to **stdout**
3. Has a **5-second timeout** by default (configurable)
4. Receives context via environment variables (see below)

#### Example Handler (Shell)

```bash
#!/bin/sh
# handlers/refresh.sh — /x/my-plugin/refresh

INPUT=$(cat)
TIMESTAMP=$(date +%s)

cat <<EOF
{
  "ok": true,
  "timestamp": ${TIMESTAMP},
  "plugin": "${BROWSERCLI_PLUGIN_NAME}"
}
EOF
```

#### Example Handler (Python)

```python
#!/usr/bin/env python3
import json
import os
import sys

request = json.loads(sys.stdin.read() or "{}")

response = {
    "ok": True,
    "plugin": os.environ.get("BROWSERCLI_PLUGIN_NAME", "unknown"),
    "echo": request,
}

print(json.dumps(response))
```

#### Example Handler (Node.js)

```javascript
#!/usr/bin/env node
const chunks = [];
process.stdin.on("data", (c) => chunks.push(c));
process.stdin.on("end", () => {
  const request = JSON.parse(Buffer.concat(chunks).toString() || "{}");
  const response = {
    ok: true,
    plugin: process.env.BROWSERCLI_PLUGIN_NAME || "unknown",
    echo: request,
  };
  console.log(JSON.stringify(response));
});
```

### Calling Plugin Endpoints

From the CLI:

```bash
# Using curl (find the HTTP port from `browsercli status`)
curl -X POST http://127.0.0.1:<port>/x/my-plugin/refresh \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{"key": "value"}'
```

From Node.js:

```typescript
const ac = BrowserCLI.connect();
const result = await ac.pluginRpc("/x/my-plugin/refresh", { key: "value" });
```

From Python:

```python
ac = BrowserCLI.connect()
result = ac.plugin_rpc("/x/my-plugin/refresh", {"key": "value"})
```

## Lifecycle Hooks

Hooks are scripts that run in response to daemon events. They are fire-and-forget: errors are logged but never crash the daemon.

### Supported Events

| Event | Trigger | Extra Environment Variables |
| --- | --- | --- |
| `on_daemon_start` | Daemon starts and browser is ready | *(none)* |
| `on_daemon_stop` | Daemon is shutting down | *(none)* |
| `on_file_change` | File changed in serve directory | `BROWSERCLI_FILE_PATH` |
| `on_navigate` | Browser navigates to a new URL | `BROWSERCLI_URL` |
| `on_console` | Console message emitted | JSON on stdin |
| `on_network` | Network request completed | JSON on stdin |

### Hook Script

Hook scripts:

- Run asynchronously (daemon does not wait for completion)
- Have a **5-second timeout**
- Receive context via environment variables
- For `on_console` and `on_network`, receive event data as JSON on stdin
- Multiple plugins can register hooks for the same event (all run)

#### Example Hook

```bash
#!/bin/sh
# hooks/on_start.sh — runs when daemon starts

echo "[my-plugin] Daemon started on port ${BROWSERCLI_HTTP_PORT}"
echo "[my-plugin] Serving from ${BROWSERCLI_DIR}"
```

## Environment Variables

All plugin scripts (hooks, handlers) receive these environment variables:

| Variable | Description |
| --- | --- |
| `BROWSERCLI_TOKEN` | Bearer token for RPC authentication |
| `BROWSERCLI_HTTP_PORT` | HTTP server port |
| `BROWSERCLI_DIR` | Path to the serve directory |
| `BROWSERCLI_BASE_URL` | Full base URL (e.g., `http://127.0.0.1:8080`) |
| `BROWSERCLI_STATE_DIR` | Path to the state directory (`~/.browsercli/`) |
| `BROWSERCLI_PLUGIN_NAME` | Name of the current plugin |

Additional event-specific variables are listed in the hooks table above.

## Cross-Platform Notes

### Script Interpreters

On **Unix** (macOS/Linux), scripts use their shebang line (`#!/bin/sh`, `#!/usr/bin/env python3`, etc.).

On **Windows**, the executor maps file extensions to interpreters:

| Extension | Interpreter |
| --- | --- |
| `.sh` | `bash` (Git Bash, WSL, or MSYS2) |
| `.py` | `python` |
| `.js` | `node` |
| `.ps1` | `powershell` |
| `.bat`, `.cmd` | `cmd /C` |

### Recommendations

- Use `#!/bin/sh` for maximum portability
- For cross-platform plugins, consider Python or Node.js scripts
- Mark scripts as executable on Unix: `chmod +x handlers/*.sh hooks/*.sh`
- Test on all target platforms

## Security Model

Plugins run with the same permissions as the browsercli daemon process. The following security measures are in place:

- **Path traversal prevention**: script paths cannot contain `..` or be absolute
- **Namespace isolation**: all plugin RPC endpoints must use the `/x/` prefix
- **Timeout enforcement**: scripts are killed after the timeout expires
- **Template source validation**: template directories must be relative to the plugin directory
- **No network access control**: scripts can make network requests — only install trusted plugins

### Best Practices

- Only install plugins from trusted sources
- Review `plugin.json` and handler scripts before installation
- Keep plugin scripts simple and focused
- Use the principle of least privilege in handler scripts
- Do not store secrets in `plugin.json`

## Testing Plugins

### Manual Testing

```bash
# Install your plugin
cp -r my-plugin ~/.browsercli/plugins/

# Verify it loads
browsercli plugin list

# Test a template
browsercli start --template my-template

# Test an RPC endpoint (find port from status)
browsercli status --json | jq .http_port
curl -X POST http://127.0.0.1:<port>/x/my-plugin/action \
  -H "Authorization: Bearer $(cat ~/.browsercli/session.json | jq -r .token)"
```

### Testing Handler Scripts Directly

You can test handler scripts without running browsercli:

```bash
# Test a handler script
echo '{"key":"value"}' | BROWSERCLI_PLUGIN_NAME=test ./handlers/refresh.sh

# Test a hook script
BROWSERCLI_HTTP_PORT=8080 BROWSERCLI_DIR=/tmp/test ./hooks/on_start.sh
```

## FAQ

**Q: Can I use any programming language for plugin scripts?**

A: Yes. Any executable script works. The daemon spawns a child process and communicates via stdin/stdout. Use the shebang line (Unix) or appropriate file extension (Windows).

**Q: What happens if a hook script fails?**

A: Hook failures are logged but never crash the daemon. The hook event is fire-and-forget.

**Q: What happens if an RPC handler script fails?**

A: The daemon returns an HTTP 500 error with the script's stderr content.

**Q: Can multiple plugins define the same template name?**

A: Yes, but the last plugin loaded wins. A warning is logged for conflicts.

**Q: Can multiple plugins register hooks for the same event?**

A: Yes. All registered hooks run (in parallel via `tokio::spawn`).

**Q: What is the script execution timeout?**

A: 5 seconds by default. Scripts exceeding this are killed.

**Q: Where do plugins store state?**

A: Plugins can use the serve directory (`BROWSERCLI_DIR`), the state directory (`BROWSERCLI_STATE_DIR`), or any location accessible to the user. There is no dedicated per-plugin state directory.

## Example

See [`examples/plugins/dashboard/`](examples/plugins/dashboard/) for a complete example plugin with a template, RPC handlers, and lifecycle hooks.
