#!/usr/bin/env python3
"""Example 3 — Collect console, network, and perf data to produce a debug report.

Usage:
    browsercli start --dir /tmp/demo-site
    python examples/03_debug_report.py
    browsercli stop
"""

import json
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
        info = ac.status()
        serve_dir = info.get("dir", "")
        if not serve_dir:
            print("ERROR: daemon has no serve directory")
            return

        # 1. Write a page that produces console output and loads resources
        html = """\
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Debug Page</title>
  <link rel="stylesheet" href="style.css">
</head>
<body>
  <h1>Debug Demo</h1>
  <script>
    console.log("page loaded");
    console.warn("this is a warning");
    console.error("simulated error for testing");
    fetch("/api/data").catch(function() {
      console.error("fetch failed as expected");
    });
  </script>
</body>
</html>
"""
        css = "body { font-family: sans-serif; margin: 2rem; }\n"

        Path(serve_dir, "index.html").write_text(html)
        Path(serve_dir, "style.css").write_text(css)
        time.sleep(1)

        # 2. Navigate and let the page settle
        ac.goto("/")
        time.sleep(2)

        # 3. Collect diagnostics
        console_entries = ac.console()
        network_entries = ac.network()
        perf_data = ac.perf()
        status_data = ac.status()

        # 4. Print summary
        print(f"Console entries: {len(console_entries)}")
        for e in console_entries:
            print(f"  [{e['level']}] {e['text']}")

        print(f"\nNetwork entries: {len(network_entries)}")
        for e in network_entries:
            print(
                f"  {e['method']} {e['status']} {e['url']} "
                f"({e['duration_ms']}ms, {e['size']}B)"
            )

        print(f"\nPerformance:")
        print(f"  DOMContentLoaded: {perf_data.get('dom_content_loaded_ms', 0):.1f}ms")
        print(f"  Load:             {perf_data.get('load_event_ms', 0):.1f}ms")

        # 5. Build and save full report
        report = {
            "status": status_data,
            "console": console_entries,
            "network": network_entries,
            "performance": perf_data,
        }
        out_path = str(Path(serve_dir) / "debug_report.json")
        Path(out_path).write_text(json.dumps(report, indent=2))
        print(f"\nFull report saved to {out_path}")

    except (ConnectionError, ServerError) as e:
        print(f"ERROR: {e}")


if __name__ == "__main__":
    main()
