#!/usr/bin/env python3
"""Example 2 — Auto-fill a form, click submit, wait for network, export results.

Usage:
    browsercli start --dir /tmp/demo-site
    python examples/02_form_fill_and_submit.py
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

        # 1. Write a page with a form that "submits" via JS (no real server needed)
        html = """\
<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Form Demo</title></head>
<body>
  <h1>Registration</h1>
  <form id="reg-form" onsubmit="return handleSubmit(event)">
    <input id="name" type="text" placeholder="Name" />
    <input id="email" type="email" placeholder="Email" />
    <button id="submit-btn" type="submit">Submit</button>
  </form>
  <div id="result" style="display:none"></div>
  <script>
    function handleSubmit(e) {
      e.preventDefault();
      var name = document.getElementById('name').value;
      var email = document.getElementById('email').value;
      // Simulate an API call with fetch to a local echo
      fetch('/echo?name=' + encodeURIComponent(name) + '&email=' + encodeURIComponent(email))
        .catch(function() {})  // will 404, that's fine
        .finally(function() {
          var el = document.getElementById('result');
          el.textContent = 'Submitted: ' + name + ' <' + email + '>';
          el.style.display = 'block';
          console.log('form-submitted', {name: name, email: email});
        });
      return false;
    }
  </script>
</body>
</html>
"""
        Path(serve_dir, "index.html").write_text(html)
        time.sleep(1)
        ac.goto("/")
        time.sleep(0.5)

        # 2. Fill the form
        ac.dom_type("#name", "Alice Agent", clear=True)
        ac.dom_type("#email", "alice@example.com", clear=True)
        print("Filled form fields")

        # 3. Clear network log, then click submit
        ac.network(clear=True)
        ac.dom_click("#submit-btn")
        time.sleep(1)

        # 4. Check the result text appeared
        result_text = ac.dom_query("#result", mode="text")
        print(f"Result element: {result_text}")

        # 5. Inspect network log for the fetch call
        entries = ac.network()
        print(f"Network entries after submit: {len(entries)}")
        for entry in entries:
            print(f"  {entry['method']} {entry['url']} -> {entry['status']}")

        # 6. Export results as JSON
        report = {
            "form_result": result_text,
            "network_requests": entries,
        }
        out_path = str(Path(serve_dir) / "form_report.json")
        Path(out_path).write_text(json.dumps(report, indent=2))
        print(f"Report saved to {out_path}")

    except (ConnectionError, ServerError) as e:
        print(f"ERROR: {e}")


if __name__ == "__main__":
    main()
