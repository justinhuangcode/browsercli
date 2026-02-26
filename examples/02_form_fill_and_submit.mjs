#!/usr/bin/env node
/**
 * Example 2 — Auto-fill a form, click submit, wait for network, export results.
 *
 * Usage:
 *     browsercli start --dir /tmp/demo-site
 *     cd clients/node && npm run build && cd ../..
 *     node examples/02_form_fill_and_submit.mjs
 *     browsercli stop
 */

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { setTimeout } from "node:timers/promises";

import {
  BrowserCLI,
  SessionError,
  ConnectionError,
  ServerError,
} from "../clients/node/dist/src/index.js";

async function main() {
  let ac;
  try {
    ac = BrowserCLI.connect();
  } catch (err) {
    if (err instanceof SessionError) {
      console.log(`ERROR: ${err.message}`);
      console.log(
        "Hint: start the daemon first with: browsercli start --dir /tmp/demo-site"
      );
      return;
    }
    throw err;
  }

  try {
    const info = await ac.status();
    const serveDir = info.dir;
    if (!serveDir) {
      console.log("ERROR: daemon has no serve directory");
      return;
    }

    // 1. Write a page with a form that "submits" via JS (no real server needed)
    const html = `<!DOCTYPE html>
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
`;
    writeFileSync(join(serveDir, "index.html"), html);
    await setTimeout(1000);
    await ac.goto("/");
    await setTimeout(500);

    // 2. Fill the form
    await ac.domType("#name", "Alice Agent", true);
    await ac.domType("#email", "alice@example.com", true);
    console.log("Filled form fields");

    // 3. Clear network log, then click submit
    await ac.network(0, true);
    await ac.domClick("#submit-btn");
    await setTimeout(1000);

    // 4. Check the result text appeared
    const resultText = await ac.domQuery("#result", "text");
    console.log(`Result element: ${resultText}`);

    // 5. Inspect network log for the fetch call
    const entries = await ac.network();
    console.log(`Network entries after submit: ${entries.length}`);
    for (const e of entries) {
      console.log(`  ${e.method} ${e.url} -> ${e.status}`);
    }

    // 6. Export results as JSON
    const report = {
      form_result: resultText,
      network_requests: entries,
    };
    const outPath = join(serveDir, "form_report.json");
    writeFileSync(outPath, JSON.stringify(report, null, 2));
    console.log(`Report saved to ${outPath}`);
  } catch (err) {
    if (err instanceof ConnectionError || err instanceof ServerError) {
      console.log(`ERROR: ${err.message}`);
    } else {
      throw err;
    }
  }
}

main();
