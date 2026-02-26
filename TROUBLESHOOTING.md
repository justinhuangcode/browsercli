# Troubleshooting

Common issues and solutions for browsercli.

## Browser Not Found

**Error:** `could not find a Chromium-based browser`

browsercli requires a Chromium-based browser (Chrome, Chromium, Brave, or Edge).

### Install a browser

**macOS:**
```bash
brew install --cask google-chrome
# or
brew install --cask chromium
```

**Ubuntu / Debian:**
```bash
sudo apt install chromium-browser
```

**Fedora:**
```bash
sudo dnf install chromium
```

**Arch Linux:**
```bash
sudo pacman -S chromium
```

**Snap:**
```bash
snap install chromium
```

**Windows:**
```powershell
winget install Google.Chrome
# or
choco install googlechrome
```

### Use a custom browser path

If your browser is installed in a non-standard location:

```bash
browsercli start --browser-bin /path/to/chrome
```

Or set the `CHROME_BIN` environment variable:

```bash
export CHROME_BIN=/path/to/chrome
browsercli start
```

## Port Already in Use

**Error:** `Address already in use`

Another process is using the port. Either:

1. **Let browsercli pick a free port** (default behavior — use `--port 0`):
   ```bash
   browsercli start
   ```

2. **Specify a different port:**
   ```bash
   browsercli start --port 9090
   ```

3. **Find and stop the conflicting process:**
   ```bash
   lsof -i :8080        # macOS / Linux
   netstat -ano | findstr :8080  # Windows
   ```

## "Already Running" Error

**Error:** `browsercli is already running (PID ...)`

A previous session is still active. Either:

```bash
# Stop the existing session
browsercli stop

# Or restart with --restart flag
browsercli start --restart
```

If `browsercli stop` doesn't work (e.g., stale session file):

```bash
# Remove the session file manually
rm ~/.browsercli/session.json

# Then start again
browsercli start
```

## Headless Mode Issues

If you're running browsercli on a server or CI environment without a display:

```bash
browsercli start --headless
```

Common issues in headless mode:

- **No GPU acceleration:** This is expected. Chromium will use software rendering.
- **Snap Chromium sandbox errors on Linux:** Try `CHROME_FLAGS="--no-sandbox"` (only in trusted environments).

## Permission Denied (Unix Socket)

**Error:** `Permission denied` when connecting to the daemon

The socket file at `~/.browsercli/browsercli.sock` may have incorrect permissions:

```bash
rm ~/.browsercli/browsercli.sock
browsercli start
```

## Template Not Found

**Error:** `template 'xxx' not found`

Check available templates:

```bash
# Built-in templates
browsercli start --template tailwind
browsercli start --template dashboard
browsercli start --template chart
browsercli start --template form

# List plugin-provided templates
browsercli plugin list
```

## Screenshot Returns Blank Image

This usually happens when:

1. **The page hasn't finished loading.** Add a short delay or wait for an element:
   ```bash
   browsercli dom wait "body" --state visible
   browsercli screenshot --out page.png
   ```

2. **The browser window is too small.** Increase the window size:
   ```bash
   browsercli start --window-size 1920,1080
   ```

## Daemon Log

If something goes wrong during startup, check the daemon log:

```bash
cat ~/.browsercli/daemon.log
```

On Windows:
```powershell
type %LOCALAPPDATA%\browsercli\daemon.log
```

## Still Need Help?

- [Open an issue](https://github.com/justinhuangcode/browsercli/issues)
- [Start a discussion](https://github.com/justinhuangcode/browsercli/discussions)
