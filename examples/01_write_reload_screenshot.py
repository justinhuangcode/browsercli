#!/usr/bin/env python3
"""Example 1 — Agent writes a page, auto-reload picks it up, then screenshot.

Usage:
    browsercli start --dir /tmp/demo-site
    python examples/01_write_reload_screenshot.py
    browsercli stop
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "clients" / "python"))

from browsercli import BrowserCLI, SessionError, ConnectionError, ServerError


def main() -> None:
    try:
        ac = BrowserCLI.connect()
    except SessionError as e:
        print(f"ERROR: {e}")
        print("Hint: start the daemon first with: browsercli start --dir /tmp/demo-site")
        return

    try:
        # 1. Resolve the serve directory from daemon status
        info = ac.status()
        serve_dir = info.get("dir", "")
        if not serve_dir:
            print("ERROR: daemon has no serve directory")
            return
        print(f"Serve directory: {serve_dir}")

        # 2. Write an HTML file into the serve directory
        html = """\
<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Agent Demo</title></head>
<body>
  <h1 id="title">Hello from the Agent</h1>
  <p>Generated at {timestamp}</p>
</body>
</html>
""".format(timestamp=time.strftime("%Y-%m-%d %H:%M:%S"))

        index_path = Path(serve_dir) / "index.html"
        index_path.write_text(html)
        print(f"Wrote {index_path}")

        # 3. Wait for auto-reload to pick up the change
        time.sleep(1)

        # 4. Navigate and verify
        ac.goto("/")
        time.sleep(0.5)
        title = ac.dom_query("#title", mode="text")
        print(f"Page title text: {title}")

        # 5. Take a screenshot
        out_path = str(Path(serve_dir) / "screenshot.png")
        ac.screenshot(out=out_path)
        print(f"Screenshot saved to {out_path}")

    except (ConnectionError, ServerError) as e:
        print(f"ERROR: {e}")


if __name__ == "__main__":
    main()
