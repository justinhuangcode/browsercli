#!/usr/bin/env node
/**
 * Example 1 — Agent writes a page, auto-reload picks it up, then screenshot.
 *
 * Usage:
 *     browsercli start --dir /tmp/demo-site
 *     cd clients/node && npm run build && cd ../..
 *     node examples/01_write_reload_screenshot.mjs
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
    // 1. Resolve the serve directory from daemon status
    const info = await ac.status();
    const serveDir = info.dir;
    if (!serveDir) {
      console.log("ERROR: daemon has no serve directory");
      return;
    }
    console.log(`Serve directory: ${serveDir}`);

    // 2. Write an HTML file into the serve directory
    const timestamp = new Date().toISOString().replace("T", " ").slice(0, 19);
    const html = `<!DOCTYPE html>
<html lang="en">
<head><meta charset="utf-8"><title>Agent Demo</title></head>
<body>
  <h1 id="title">Hello from the Agent</h1>
  <p>Generated at ${timestamp}</p>
</body>
</html>
`;
    const indexPath = join(serveDir, "index.html");
    writeFileSync(indexPath, html);
    console.log(`Wrote ${indexPath}`);

    // 3. Wait for auto-reload to pick up the change
    await setTimeout(1000);

    // 4. Navigate and verify
    await ac.goto("/");
    await setTimeout(500);
    const title = await ac.domQuery("#title", "text");
    console.log(`Page title text: ${title}`);

    // 5. Take a screenshot
    const outPath = join(serveDir, "screenshot.png");
    await ac.screenshot("", outPath);
    console.log(`Screenshot saved to ${outPath}`);
  } catch (err) {
    if (err instanceof ConnectionError || err instanceof ServerError) {
      console.log(`ERROR: ${err.message}`);
    } else {
      throw err;
    }
  }
}

main();
