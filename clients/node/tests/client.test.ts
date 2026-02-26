/**
 * Comprehensive tests for the browsercli Node.js client.
 *
 * Tests are grouped into:
 * - Session file parsing (no server)
 * - Constructor validation (no server)
 * - Parameter validation (no server)
 * - Contract tests with a mock HTTP server (Unix socket or TCP on Windows)
 * - Error handling with mock server
 * - Exception hierarchy
 * - toString and constants
 *
 * Uses Node.js built-in `node:test` runner (Node 18+).
 */

import { describe, it, before, after, beforeEach, afterEach } from "node:test";
import * as assert from "node:assert/strict";
import * as http from "node:http";
import * as net from "node:net";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";

import { BrowserCLI } from "../src/client.js";
import {
  BrowserCLIError,
  AuthenticationError,
  BadRequestError,
  ConnectionError,
  NotFoundError,
  RPCError,
  ServerError,
  SessionError,
} from "../src/errors.js";
import { DOM_MODES, WAIT_STATES, CONSOLE_LEVELS } from "../src/constants.js";
import { VERSION } from "../src/index.js";

// ======================================================================
// Mock Unix Socket Server
// ======================================================================

const MOCK_TOKEN = "deadbeef01234567deadbeef01234567";

const MOCK_RESPONSES: Record<string, { status: number; body: unknown }> = {
  "GET /version": {
    status: 200,
    body: { rpc_version: 1, schema_version: 1 },
  },
  "GET /status": {
    status: 200,
    body: {
      running: true,
      browser_alive: true,
      pid: 12345,
      dir: "/tmp/serve",
      http_addr: "127.0.0.1",
      http_port: 8080,
      current_url: "http://127.0.0.1:8080/",
      title: "Test Page",
      headless: true,
      browser_pid: 12346,
      devtools_port: 9222,
      browser_bin: "/usr/bin/chromium",
    },
  },
  "POST /goto": {
    status: 200,
    body: { url: "http://127.0.0.1:8080/about", title: "About" },
  },
  "POST /eval": {
    status: 200,
    body: { value: 42 },
  },
  "POST /reload": {
    status: 200,
    body: { ok: true },
  },
  "POST /dom": {
    status: 200,
    body: { selector: "h1", mode: "text", value: "Hello" },
  },
  "POST /dom/all": {
    status: 200,
    body: { selector: "p", mode: "text", values: ["a", "b", "c"] },
  },
  "POST /dom/attr": {
    status: 200,
    body: { selector: "a", name: "href", value: "/about" },
  },
  "POST /dom/click": {
    status: 200,
    body: { ok: true },
  },
  "POST /dom/type": {
    status: 200,
    body: { ok: true },
  },
  "POST /dom/wait": {
    status: 200,
    body: { ok: true, state: "visible" },
  },
  "POST /screenshot": {
    status: 200,
    body: {
      format: "png",
      base64: Buffer.from([
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG header
        0x66, 0x61, 0x6b, 0x65, // "fake"
      ]).toString("base64"),
    },
  },
  "POST /console": {
    status: 200,
    body: {
      entries: [
        { level: "log", text: "hello", timestamp: 1000 },
        { level: "error", text: "boom", timestamp: 2000 },
      ],
    },
  },
  "POST /network": {
    status: 200,
    body: {
      entries: [
        {
          method: "GET",
          url: "http://127.0.0.1:8080/",
          status: 200,
          resource_type: "Document",
          mime_type: "text/html",
          size: 1024,
          duration_ms: 50,
          timestamp: 3000,
        },
      ],
    },
  },
  "GET /perf": {
    status: 200,
    body: { dom_content_loaded_ms: 150.5, load_event_ms: 300.2 },
  },
  "POST /stop": {
    status: 200,
    body: { ok: true },
  },
  "GET /plugins": {
    status: 200,
    body: {
      plugins: [
        {
          name: "test-plugin",
          version: "1.0.0",
          description: "A test plugin",
          templates: ["dashboard"],
          hooks: ["on_daemon_start"],
          rpc_endpoints: ["/x/test-plugin/hello"],
        },
      ],
    },
  },
  "POST /x/test-plugin/hello": {
    status: 200,
    body: { plugin: "test-plugin", message: "hello!" },
  },
};

/**
 * Override response for the mock server. Set to a tuple of
 * [statusCode, body] to override the default behavior, or null to use defaults.
 */
let overrideResponse: { status: number; body: unknown } | null = null;

/**
 * Create a mock HTTP server emulating the browsercli Rust daemon's RPC API.
 * The same server instance works for both Unix socket and TCP listeners.
 */
function createMockServer(): http.Server {
  const server = http.createServer((req, res) => {
    // Auth check — matches server.rs behavior.
    const auth = req.headers.authorization ?? "";
    if (auth !== `Bearer ${MOCK_TOKEN}`) {
      // The Rust daemon returns plain text "unauthorized" on 401.
      res.writeHead(401, { "Content-Type": "text/plain" });
      res.end("unauthorized");
      return;
    }

    // Read request body.
    let body = "";
    req.on("data", (chunk: Buffer) => {
      body += chunk.toString();
    });
    req.on("end", () => {
      // Check for override.
      if (overrideResponse !== null) {
        const { status, body: respBody } = overrideResponse;
        res.writeHead(status, { "Content-Type": "application/json" });
        res.end(JSON.stringify(respBody));
        return;
      }

      const key = `${req.method} ${req.url}`;
      const mock = MOCK_RESPONSES[key];

      if (mock) {
        res.writeHead(mock.status, { "Content-Type": "application/json" });
        res.end(JSON.stringify(mock.body));
      } else {
        // 404 — matches server.rs: (404, json!({"error": "not found"}))
        res.writeHead(404, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: "not found" }));
      }
    });
  });

  return server;
}

/**
 * Helper: start a mock server and return cleanup helpers.
 *
 * On Unix the server listens on a Unix domain socket.
 * On Windows (or when `forceTcp` is true) it listens on TCP localhost.
 */
interface MockServerHandle {
  /** Unix socket path (Unix) or `host:port` string (Windows/TCP). */
  addr: string;
  server: http.Server;
  client: (token?: string) => BrowserCLI;
  close: () => Promise<void>;
}

async function startMockServer(forceTcp: boolean = false): Promise<MockServerHandle> {
  const useTcp = process.platform === "win32" || forceTcp;
  const server = createMockServer();

  if (useTcp) {
    // TCP mode.
    await new Promise<void>((resolve) => {
      server.listen(0, "127.0.0.1", () => resolve());
    });
    const address = server.address() as net.AddressInfo;
    const addr = `127.0.0.1:${address.port}`;

    return {
      addr,
      server,
      client: (token?: string) =>
        new BrowserCLI(addr, token ?? MOCK_TOKEN, 5000),
      close: () =>
        new Promise<void>((resolve) => {
          server.close(() => resolve());
        }),
    };
  }

  // Unix socket mode.
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-test-"));
  const socketPath = path.join(tmpDir, "test.sock");

  await new Promise<void>((resolve) => {
    server.listen(socketPath, () => resolve());
  });

  return {
    addr: socketPath,
    server,
    client: (token?: string) =>
      new BrowserCLI(socketPath, token ?? MOCK_TOKEN, 5000),
    close: () =>
      new Promise<void>((resolve) => {
        server.close(() => {
          try {
            fs.unlinkSync(socketPath);
          } catch {}
          try {
            fs.rmdirSync(tmpDir);
          } catch {}
          resolve();
        });
      }),
  };
}

// ======================================================================
// Test: Session file parsing
// ======================================================================

describe("BrowserCLI.connect() — session file parsing", () => {
  it("reads a valid session file", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-session-"));
    const sessionPath = path.join(tmpDir, "session.json");
    fs.writeFileSync(
      sessionPath,
      JSON.stringify({ socket_path: "/tmp/test.sock", token: "abc123" })
    );

    try {
      const ac = BrowserCLI.connect(sessionPath);
      // Access private fields to verify parsing.
      assert.equal((ac as any)._addr, "/tmp/test.sock");
      assert.equal((ac as any)._token, "abc123");
    } finally {
      fs.unlinkSync(sessionPath);
      fs.rmdirSync(tmpDir);
    }
  });

  it("throws SessionError for missing file", () => {
    assert.throws(
      () => BrowserCLI.connect("/tmp/nonexistent-session-12345.json"),
      (err: unknown) => {
        assert.ok(err instanceof SessionError);
        assert.ok(err instanceof BrowserCLIError);
        assert.match(err.message, /not found/i);
        assert.match(err.message, /daemon running/i);
        return true;
      }
    );
  });

  it("throws SessionError for empty token", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-session-"));
    const sessionPath = path.join(tmpDir, "session.json");
    fs.writeFileSync(
      sessionPath,
      JSON.stringify({ socket_path: "/tmp/test.sock", token: "" })
    );

    try {
      assert.throws(
        () => BrowserCLI.connect(sessionPath),
        (err: unknown) => err instanceof SessionError
      );
    } finally {
      fs.unlinkSync(sessionPath);
      fs.rmdirSync(tmpDir);
    }
  });

  it("throws SessionError for missing socket_path and rpc_port", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-session-"));
    const sessionPath = path.join(tmpDir, "session.json");
    fs.writeFileSync(sessionPath, JSON.stringify({ token: "abc" }));

    try {
      assert.throws(
        () => BrowserCLI.connect(sessionPath),
        (err: unknown) => err instanceof SessionError
      );
    } finally {
      fs.unlinkSync(sessionPath);
      fs.rmdirSync(tmpDir);
    }
  });

  it("reads a session file with rpc_port (Windows format)", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-session-"));
    const sessionPath = path.join(tmpDir, "session.json");
    fs.writeFileSync(
      sessionPath,
      JSON.stringify({ rpc_port: 12345, token: "abc123" })
    );

    try {
      const ac = BrowserCLI.connect(sessionPath);
      assert.equal((ac as any)._addr, "127.0.0.1:12345");
      assert.equal((ac as any)._token, "abc123");
      assert.equal((ac as any)._useTcp, true);
    } finally {
      fs.unlinkSync(sessionPath);
      fs.rmdirSync(tmpDir);
    }
  });

  it("throws SessionError for invalid JSON", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-session-"));
    const sessionPath = path.join(tmpDir, "session.json");
    fs.writeFileSync(sessionPath, "not valid json {{{");

    try {
      assert.throws(
        () => BrowserCLI.connect(sessionPath),
        (err: unknown) => err instanceof SessionError
      );
    } finally {
      fs.unlinkSync(sessionPath);
      fs.rmdirSync(tmpDir);
    }
  });

  it("throws SessionError for JSON array", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-session-"));
    const sessionPath = path.join(tmpDir, "session.json");
    fs.writeFileSync(sessionPath, JSON.stringify([1, 2, 3]));

    try {
      assert.throws(
        () => BrowserCLI.connect(sessionPath),
        (err: unknown) => err instanceof SessionError
      );
    } finally {
      fs.unlinkSync(sessionPath);
      fs.rmdirSync(tmpDir);
    }
  });

  it("uses default path when no sessionPath given", () => {
    // Since ~/.browsercli/session.json likely doesn't exist in test env,
    // this should throw SessionError.
    assert.throws(
      () => BrowserCLI.connect(),
      (err: unknown) => err instanceof SessionError
    );
  });
});

// ======================================================================
// Test: Constructor validation
// ======================================================================

describe("BrowserCLI constructor validation", () => {
  it("throws TypeError for empty addr", () => {
    assert.throws(
      () => new BrowserCLI("", "token"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("throws TypeError for empty token", () => {
    assert.throws(
      () => new BrowserCLI("/tmp/test.sock", ""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("throws TypeError for negative timeout", () => {
    assert.throws(
      () => new BrowserCLI("/tmp/test.sock", "token", -1),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("throws TypeError for zero timeout", () => {
    assert.throws(
      () => new BrowserCLI("/tmp/test.sock", "token", 0),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("accepts valid Unix socket parameters", () => {
    const ac = new BrowserCLI("/tmp/test.sock", "token123", 5000);
    assert.equal((ac as any)._addr, "/tmp/test.sock");
    assert.equal((ac as any)._token, "token123");
    assert.equal((ac as any)._timeout, 5000);
    assert.equal((ac as any)._useTcp, false);
  });

  it("accepts valid TCP address parameters", () => {
    const ac = new BrowserCLI("127.0.0.1:12345", "token123", 5000);
    assert.equal((ac as any)._addr, "127.0.0.1:12345");
    assert.equal((ac as any)._useTcp, true);
  });

  it("uses default timeout of 30000", () => {
    const ac = new BrowserCLI("/tmp/test.sock", "token");
    assert.equal((ac as any)._timeout, 30000);
  });
});

// ======================================================================
// Test: Parameter validation (client-side, no server needed)
// ======================================================================

describe("Parameter validation (no server)", () => {
  const ac = new BrowserCLI("/tmp/fake.sock", "tok");

  it("goto: rejects non-string url", async () => {
    await assert.rejects(
      () => ac.goto(123 as any),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("eval: rejects empty string", async () => {
    await assert.rejects(
      () => ac.eval(""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("eval: rejects whitespace-only string", async () => {
    await assert.rejects(
      () => ac.eval("   "),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domQuery: rejects empty selector", async () => {
    await assert.rejects(
      () => ac.domQuery(""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domQuery: rejects invalid mode", async () => {
    await assert.rejects(
      () => ac.domQuery("h1", "innerHTML"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domAll: rejects empty selector", async () => {
    await assert.rejects(
      () => ac.domAll(""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domAll: rejects invalid mode", async () => {
    await assert.rejects(
      () => ac.domAll("p", "bad"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domAttr: rejects empty selector", async () => {
    await assert.rejects(
      () => ac.domAttr("", "href"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domAttr: rejects empty attribute name", async () => {
    await assert.rejects(
      () => ac.domAttr("a", ""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domClick: rejects empty selector", async () => {
    await assert.rejects(
      () => ac.domClick(""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domType: rejects empty selector", async () => {
    await assert.rejects(
      () => ac.domType("", "text"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domType: rejects non-string text", async () => {
    await assert.rejects(
      () => ac.domType("input", 123 as any),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domWait: rejects empty selector", async () => {
    await assert.rejects(
      () => ac.domWait(""),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domWait: rejects invalid state", async () => {
    await assert.rejects(
      () => ac.domWait("div", "gone"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domWait: rejects negative timeout", async () => {
    await assert.rejects(
      () => ac.domWait("div", "visible", -1),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domWait: rejects zero timeout", async () => {
    await assert.rejects(
      () => ac.domWait("div", "visible", 0),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("domWait: rejects non-integer timeout", async () => {
    await assert.rejects(
      () => ac.domWait("div", "visible", 1.5),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("console: rejects invalid level", async () => {
    await assert.rejects(
      () => ac.console("debug"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("console: rejects negative limit", async () => {
    await assert.rejects(
      () => ac.console("", -1),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("network: rejects negative limit", async () => {
    await assert.rejects(
      () => ac.network(-1),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("screenshot: rejects non-string selector", async () => {
    await assert.rejects(
      () => ac.screenshot(123 as any),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("pluginRpc: rejects path not starting with /x/", async () => {
    await assert.rejects(
      () => ac.pluginRpc("/status"),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("pluginRpc: rejects non-string path", async () => {
    await assert.rejects(
      () => ac.pluginRpc(123 as any),
      (err: unknown) => err instanceof TypeError
    );
  });

  it("pluginRpc: rejects empty path", async () => {
    await assert.rejects(
      () => ac.pluginRpc(""),
      (err: unknown) => err instanceof TypeError
    );
  });
});

// ======================================================================
// Test: Contract tests with mock server
// ======================================================================

describe("Contract tests with mock server", () => {
  let mock: MockServerHandle;

  before(async () => {
    mock = await startMockServer();
  });

  after(async () => {
    await mock.close();
  });

  it("status — returns daemon and browser info", async () => {
    const ac = mock.client();
    const result = await ac.status();
    assert.equal(result.running, true);
    assert.equal(result.browser_alive, true);
    assert.equal(result.pid, 12345);
    assert.equal(result.dir, "/tmp/serve");
    assert.equal(result.http_port, 8080);
    assert.equal(result.title, "Test Page");
  });

  it("version — returns RPC and schema version", async () => {
    const ac = mock.client();
    const result = await ac.version();
    assert.equal(result.rpc_version, 1);
    assert.equal(result.schema_version, 1);
  });

  it("goto — navigates and returns url + title", async () => {
    const ac = mock.client();
    const result = await ac.goto("/about");
    assert.equal(result.url, "http://127.0.0.1:8080/about");
    assert.equal(result.title, "About");
  });

  it("eval — evaluates expression and returns value", async () => {
    const ac = mock.client();
    const result = await ac.eval("1 + 1");
    assert.equal(result, 42);
  });

  it("reload — returns true", async () => {
    const ac = mock.client();
    const result = await ac.reload();
    assert.equal(result, true);
  });

  it("domQuery — queries element and returns text", async () => {
    const ac = mock.client();
    const result = await ac.domQuery("h1", "text");
    assert.equal(result, "Hello");
  });

  it("domAll — queries all matching elements", async () => {
    const ac = mock.client();
    const result = await ac.domAll("p", "text");
    assert.deepEqual(result, ["a", "b", "c"]);
  });

  it("domAttr — returns attribute value", async () => {
    const ac = mock.client();
    const result = await ac.domAttr("a", "href");
    assert.equal(result, "/about");
  });

  it("domClick — clicks element and returns true", async () => {
    const ac = mock.client();
    const result = await ac.domClick("#btn");
    assert.equal(result, true);
  });

  it("domType — types text and returns true", async () => {
    const ac = mock.client();
    const result = await ac.domType("#input", "hello");
    assert.equal(result, true);
  });

  it("domWait — waits for element and returns true", async () => {
    const ac = mock.client();
    const result = await ac.domWait("#el");
    assert.equal(result, true);
  });

  it("screenshot — returns PNG Buffer", async () => {
    const ac = mock.client();
    const buf = await ac.screenshot();
    assert.ok(Buffer.isBuffer(buf));
    // Check PNG magic bytes.
    assert.equal(buf[0], 0x89);
    assert.equal(buf[1], 0x50); // P
    assert.equal(buf[2], 0x4e); // N
    assert.equal(buf[3], 0x47); // G
  });

  it("screenshot — writes to file when out is specified", async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "ac-ss-"));
    const outPath = path.join(tmpDir, "test.png");

    try {
      const ac = mock.client();
      const buf = await ac.screenshot("", outPath);
      assert.ok(fs.existsSync(outPath));
      const fileData = fs.readFileSync(outPath);
      assert.deepEqual(buf, fileData);
    } finally {
      try {
        fs.unlinkSync(outPath);
      } catch {}
      try {
        fs.rmdirSync(tmpDir);
      } catch {}
    }
  });

  it("console — returns console entries", async () => {
    const ac = mock.client();
    const entries = await ac.console();
    assert.equal(entries.length, 2);
    assert.equal(entries[0].level, "log");
    assert.equal(entries[0].text, "hello");
    assert.equal(entries[1].level, "error");
    assert.equal(entries[1].text, "boom");
  });

  it("network — returns network entries", async () => {
    const ac = mock.client();
    const entries = await ac.network();
    assert.equal(entries.length, 1);
    const e = entries[0];
    assert.equal(e.method, "GET");
    assert.equal(e.status, 200);
    assert.equal(e.resource_type, "Document");
    assert.equal(e.size, 1024);
    assert.equal(e.duration_ms, 50);
  });

  it("perf — returns performance metrics", async () => {
    const ac = mock.client();
    const result = await ac.perf();
    assert.equal(result.dom_content_loaded_ms, 150.5);
    assert.equal(result.load_event_ms, 300.2);
  });

  it("stop — returns true", async () => {
    const ac = mock.client();
    const result = await ac.stop();
    assert.equal(result, true);
  });

  it("pluginList — returns array of plugins", async () => {
    const ac = mock.client();
    const plugins = await ac.pluginList();
    assert.equal(plugins.length, 1);
    assert.equal(plugins[0].name, "test-plugin");
    assert.equal(plugins[0].version, "1.0.0");
    assert.equal(plugins[0].description, "A test plugin");
    assert.deepEqual(plugins[0].templates, ["dashboard"]);
    assert.deepEqual(plugins[0].hooks, ["on_daemon_start"]);
    assert.deepEqual(plugins[0].rpc_endpoints, ["/x/test-plugin/hello"]);
  });

  it("pluginRpc — calls custom endpoint", async () => {
    const ac = mock.client();
    const result = await ac.pluginRpc("/x/test-plugin/hello") as Record<string, unknown>;
    assert.equal(result.plugin, "test-plugin");
    assert.equal(result.message, "hello!");
  });

  it("pluginRpc — sends body to handler", async () => {
    const ac = mock.client();
    const result = await ac.pluginRpc("/x/test-plugin/hello", { key: "value" }) as Record<string, unknown>;
    assert.equal(result.plugin, "test-plugin");
  });
});

// ======================================================================
// Test: Error handling with mock server
// ======================================================================

describe("Error handling with mock server", () => {
  let mock: MockServerHandle;

  before(async () => {
    mock = await startMockServer();
  });

  after(async () => {
    await mock.close();
  });

  afterEach(() => {
    overrideResponse = null;
  });

  it("401 returns AuthenticationError with wrong token", async () => {
    const ac = mock.client("wrong_token");
    await assert.rejects(
      () => ac.status(),
      (err: unknown) => {
        assert.ok(err instanceof AuthenticationError);
        assert.ok(err instanceof BrowserCLIError);
        return true;
      }
    );
  });

  it("404 returns NotFoundError for unknown endpoint", async () => {
    const ac = mock.client();
    await assert.rejects(
      () => (ac as any)._request("GET", "/nonexistent"),
      (err: unknown) => {
        assert.ok(err instanceof NotFoundError);
        assert.equal((err as NotFoundError).statusCode, 404);
        assert.equal((err as NotFoundError).errorMessage, "not found");
        return true;
      }
    );
  });

  it("400 returns BadRequestError", async () => {
    overrideResponse = {
      status: 400,
      body: { error: "missing field: selector" },
    };
    const ac = mock.client();
    await assert.rejects(
      () => (ac as any)._request("POST", "/dom", { bad: true }),
      (err: unknown) => {
        assert.ok(err instanceof BadRequestError);
        assert.equal((err as BadRequestError).statusCode, 400);
        assert.ok(
          (err as BadRequestError).errorMessage.includes("selector")
        );
        return true;
      }
    );
  });

  it("500 returns ServerError", async () => {
    overrideResponse = {
      status: 500,
      body: { error: "CDP command timed out" },
    };
    const ac = mock.client();
    await assert.rejects(
      () => (ac as any)._request("POST", "/eval", { expression: "x" }),
      (err: unknown) => {
        assert.ok(err instanceof ServerError);
        assert.equal((err as ServerError).statusCode, 500);
        assert.ok((err as ServerError).errorMessage.includes("CDP"));
        return true;
      }
    );
  });

  it("non-JSON error body still yields ServerError", async () => {
    overrideResponse = { status: 500, body: "plain text error" };
    const ac = mock.client();
    await assert.rejects(
      () => (ac as any)._request("GET", "/status"),
      (err: unknown) => {
        assert.ok(err instanceof ServerError);
        return true;
      }
    );
  });

  it("connection error on missing socket/address", async () => {
    // Unix socket path that doesn't exist.
    const ac = new BrowserCLI(
      "/tmp/nonexistent-sock-12345.sock",
      "tok",
      2000
    );
    await assert.rejects(
      () => ac.status(),
      (err: unknown) => {
        assert.ok(err instanceof ConnectionError);
        assert.ok(err instanceof BrowserCLIError);
        return true;
      }
    );
  });

  it("connection error on unreachable TCP address", async () => {
    // TCP address with no listener.
    const ac = new BrowserCLI("127.0.0.1:1", "tok", 2000);
    await assert.rejects(
      () => ac.status(),
      (err: unknown) => {
        assert.ok(err instanceof ConnectionError);
        return true;
      }
    );
  });

  it("all RPC errors are catchable as BrowserCLIError", async () => {
    const ac = mock.client("wrong_token");
    await assert.rejects(
      () => ac.status(),
      (err: unknown) => {
        assert.ok(err instanceof BrowserCLIError);
        return true;
      }
    );
  });
});

// ======================================================================
// Test: Exception hierarchy
// ======================================================================

describe("Exception hierarchy", () => {
  it("all error classes inherit from BrowserCLIError", () => {
    for (const Cls of [
      ConnectionError,
      AuthenticationError,
      RPCError,
      BadRequestError,
      NotFoundError,
      ServerError,
      SessionError,
    ]) {
      const instance = Cls.length === 1 ? new (Cls as any)("test") : new (Cls as any)(500, "test");
      assert.ok(
        instance instanceof BrowserCLIError,
        `${Cls.name} should inherit BrowserCLIError`
      );
    }
  });

  it("BadRequestError is RPCError with statusCode 400", () => {
    const err = new BadRequestError("bad");
    assert.ok(err instanceof RPCError);
    assert.equal(err.statusCode, 400);
  });

  it("NotFoundError is RPCError with statusCode 404", () => {
    const err = new NotFoundError("missing");
    assert.ok(err instanceof RPCError);
    assert.equal(err.statusCode, 404);
  });

  it("ServerError is RPCError", () => {
    const err = new ServerError(500, "internal");
    assert.ok(err instanceof RPCError);
    assert.equal(err.statusCode, 500);
  });

  it("RPCError has statusCode and errorMessage", () => {
    const err = new RPCError(422, "unprocessable");
    assert.equal(err.statusCode, 422);
    assert.equal(err.errorMessage, "unprocessable");
    assert.ok(err.message.includes("422"));
    assert.ok(err.message.includes("unprocessable"));
  });
});

// ======================================================================
// Test: toString and constants
// ======================================================================

describe("toString", () => {
  it("includes addr and timeout", () => {
    const ac = new BrowserCLI("/tmp/test.sock", "secret-token", 5000);
    const str = ac.toString();
    assert.ok(str.includes("/tmp/test.sock"));
    assert.ok(str.includes("5000"));
  });

  it("includes TCP addr when using TCP", () => {
    const ac = new BrowserCLI("127.0.0.1:9999", "secret-token", 5000);
    const str = ac.toString();
    assert.ok(str.includes("127.0.0.1:9999"));
  });

  it("does NOT include token", () => {
    const ac = new BrowserCLI("/tmp/test.sock", "secret-token", 5000);
    const str = ac.toString();
    assert.ok(!str.includes("secret-token"));
  });
});

describe("Constants", () => {
  it("DOM_MODES contains expected values", () => {
    assert.ok(DOM_MODES.has("outer_html"));
    assert.ok(DOM_MODES.has("text"));
    assert.equal(DOM_MODES.size, 2);
  });

  it("WAIT_STATES contains expected values", () => {
    assert.ok(WAIT_STATES.has("visible"));
    assert.ok(WAIT_STATES.has("hidden"));
    assert.ok(WAIT_STATES.has("attached"));
    assert.ok(WAIT_STATES.has("detached"));
    assert.equal(WAIT_STATES.size, 4);
  });

  it("CONSOLE_LEVELS contains expected values", () => {
    assert.ok(CONSOLE_LEVELS.has(""));
    assert.ok(CONSOLE_LEVELS.has("log"));
    assert.ok(CONSOLE_LEVELS.has("warn"));
    assert.ok(CONSOLE_LEVELS.has("error"));
    assert.ok(CONSOLE_LEVELS.has("info"));
    assert.equal(CONSOLE_LEVELS.size, 5);
  });

  it("VERSION is a semver string", () => {
    assert.match(VERSION, /^\d+\.\d+\.\d+$/);
  });
});

// ======================================================================
// Test: TCP transport (Windows-compatible)
// ======================================================================

describe("TCP transport contract tests", () => {
  let mock: MockServerHandle;

  before(async () => {
    mock = await startMockServer(/* forceTcp */ true);
  });

  after(async () => {
    await mock.close();
  });

  it("status over TCP — returns daemon and browser info", async () => {
    const ac = mock.client();
    const result = await ac.status();
    assert.equal(result.running, true);
    assert.equal(result.pid, 12345);
  });

  it("goto over TCP — navigates and returns url + title", async () => {
    const ac = mock.client();
    const result = await ac.goto("/about");
    assert.equal(result.url, "http://127.0.0.1:8080/about");
    assert.equal(result.title, "About");
  });

  it("eval over TCP — evaluates expression", async () => {
    const ac = mock.client();
    const result = await ac.eval("1 + 1");
    assert.equal(result, 42);
  });

  it("screenshot over TCP — returns PNG Buffer", async () => {
    const ac = mock.client();
    const buf = await ac.screenshot();
    assert.ok(Buffer.isBuffer(buf));
    assert.equal(buf[0], 0x89);
    assert.equal(buf[1], 0x50);
  });

  it("stop over TCP — returns true", async () => {
    const ac = mock.client();
    const result = await ac.stop();
    assert.equal(result, true);
  });

  it("pluginList over TCP — returns plugins", async () => {
    const ac = mock.client();
    const plugins = await ac.pluginList();
    assert.equal(plugins.length, 1);
    assert.equal(plugins[0].name, "test-plugin");
  });

  it("pluginRpc over TCP — calls custom endpoint", async () => {
    const ac = mock.client();
    const result = await ac.pluginRpc("/x/test-plugin/hello") as Record<string, unknown>;
    assert.equal(result.plugin, "test-plugin");
  });

  it("401 over TCP returns AuthenticationError", async () => {
    const ac = mock.client("wrong_token");
    await assert.rejects(
      () => ac.status(),
      (err: unknown) => {
        assert.ok(err instanceof AuthenticationError);
        return true;
      }
    );
  });
});
