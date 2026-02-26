#!/usr/bin/env node
/**
 * Example 3 — Collect console, network, and perf data to produce a debug report.
 *
 * Usage:
 *     browsercli start --dir /tmp/demo-site
 *     cd clients/node && npm run build && cd ../..
 *     node examples/03_debug_report.mjs
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

    // 1. Write a page that produces console output and loads resources
    const html = `<!DOCTYPE html>
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
`;
    const css = "body { font-family: sans-serif; margin: 2rem; }\n";

    writeFileSync(join(serveDir, "index.html"), html);
    writeFileSync(join(serveDir, "style.css"), css);
    await setTimeout(1000);

    // 2. Navigate and let the page settle
    await ac.goto("/");
    await setTimeout(2000);

    // 3. Collect diagnostics
    const consoleEntries = await ac.console();
    const networkEntries = await ac.network();
    const perfData = await ac.perf();
    const statusData = await ac.status();

    // 4. Print summary
    console.log(`Console entries: ${consoleEntries.length}`);
    for (const e of consoleEntries) {
      console.log(`  [${e.level}] ${e.text}`);
    }

    console.log(`\nNetwork entries: ${networkEntries.length}`);
    for (const e of networkEntries) {
      console.log(
        `  ${e.method} ${e.status} ${e.url} (${e.duration_ms}ms, ${e.size}B)`
      );
    }

    console.log(`\nPerformance:`);
    console.log(
      `  DOMContentLoaded: ${(perfData.dom_content_loaded_ms ?? 0).toFixed(1)}ms`
    );
    console.log(`  Load:             ${(perfData.load_event_ms ?? 0).toFixed(1)}ms`);

    // 5. Build and save full report
    const report = {
      status: statusData,
      console: consoleEntries,
      network: networkEntries,
      performance: perfData,
    };
    const outPath = join(serveDir, "debug_report.json");
    writeFileSync(outPath, JSON.stringify(report, null, 2));
    console.log(`\nFull report saved to ${outPath}`);
  } catch (err) {
    if (err instanceof ConnectionError || err instanceof ServerError) {
      console.log(`ERROR: ${err.message}`);
    } else {
      throw err;
    }
  }
}

main();
