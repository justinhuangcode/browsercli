/**
 * Core client that talks to the browsercli daemon over its RPC API.
 *
 * On macOS/Linux the daemon listens on a Unix socket; on Windows it uses
 * TCP localhost.  The transport is chosen automatically based on the session
 * file contents.
 */

import * as http from "node:http";
import * as fs from "node:fs";
import * as path from "node:path";
import * as os from "node:os";

import {
  BrowserCLIError,
  AuthenticationError,
  BadRequestError,
  ConnectionError,
  NotFoundError,
  RPCError,
  ServerError,
  SessionError,
} from "./errors.js";
import { DOM_MODES, WAIT_STATES, CONSOLE_LEVELS } from "./constants.js";
import type {
  StatusResponse,
  VersionResponse,
  GotoResponse,
  EvalResponse,
  ReloadResponse,
  DomResponse,
  DomAllResponse,
  DomAttrResponse,
  DomClickResponse,
  DomTypeResponse,
  DomWaitResponse,
  ScreenshotResponse,
  ConsoleEntry,
  ConsoleResponse,
  NetworkEntry,
  NetworkResponse,
  PerfResponse,
  StopResponse,
  PluginInfo,
  PluginListResponse,
} from "./types.js";

/**
 * Client for a running browsercli daemon.
 *
 * @example
 * ```ts
 * import { BrowserCLI } from "@justinhuangcode/browsercli";
 *
 * const ac = BrowserCLI.connect();
 * console.log(await ac.status());
 * await ac.goto("/");
 * const text = await ac.domQuery("h1", "text");
 * await ac.stop();
 * ```
 *
 * All methods are async and return Promises.
 */
export class BrowserCLI {
  /** Unix socket path (macOS/Linux) or TCP `host:port` address (Windows). */
  private readonly _addr: string;
  private readonly _token: string;
  private readonly _timeout: number;
  /** True when connecting over TCP rather than a Unix socket. */
  private readonly _useTcp: boolean;

  /**
   * @param addr - Unix socket path **or** TCP `host:port` string.
   * @param token - Bearer token for daemon authentication.
   * @param timeout - Request timeout in milliseconds (default 30 000).
   */
  constructor(addr: string, token: string, timeout: number = 30000) {
    if (!addr || typeof addr !== "string") {
      throw new TypeError("addr must be a non-empty string");
    }
    if (!token || typeof token !== "string") {
      throw new TypeError("token must be a non-empty string");
    }
    if (typeof timeout !== "number" || timeout <= 0) {
      throw new TypeError("timeout must be a positive number");
    }
    this._addr = addr;
    this._token = token;
    this._timeout = timeout;
    // Detect TCP address format (e.g. "127.0.0.1:12345").
    this._useTcp = /^\d+\.\d+\.\d+\.\d+:\d+$/.test(addr);
  }

  // ------------------------------------------------------------------
  // Factory
  // ------------------------------------------------------------------

  /**
   * Create a client by reading the daemon's session file.
   *
   * @param sessionPath - Explicit path to `session.json`.
   *   Defaults to `~/.browsercli/session.json` on macOS/Linux, or
   *   `%LOCALAPPDATA%\browsercli\session.json` on Windows.
   * @param timeout - Socket timeout in milliseconds (default 30000).
   * @throws {SessionError} If the session file is missing, unreadable,
   *   or contains invalid data.
   */
  static connect(sessionPath?: string, timeout: number = 30000): BrowserCLI {
    const actualPath =
      sessionPath ??
      (process.platform === "win32"
        ? path.join(
            process.env.LOCALAPPDATA || os.homedir(),
            "browsercli",
            "session.json"
          )
        : path.join(os.homedir(), ".browsercli", "session.json"));

    let rawData: string;
    try {
      rawData = fs.readFileSync(actualPath, "utf-8");
    } catch (err: unknown) {
      const e = err as NodeJS.ErrnoException;
      if (e.code === "ENOENT") {
        throw new SessionError(
          `Session file not found: ${actualPath} — ` +
            "is the daemon running? (browsercli start)"
        );
      }
      throw new SessionError(
        `Cannot read session file ${actualPath}: ${e.message}`
      );
    }

    let session: unknown;
    try {
      session = JSON.parse(rawData);
    } catch (err: unknown) {
      throw new SessionError(
        `Cannot parse session file ${actualPath}: ${(err as Error).message}`
      );
    }

    if (
      typeof session !== "object" ||
      session === null ||
      Array.isArray(session)
    ) {
      throw new SessionError(
        `Session file ${actualPath} does not contain a JSON object`
      );
    }

    const sess = session as Record<string, unknown>;
    const socketPathVal = sess.socket_path as string | undefined;
    const rpcPort = sess.rpc_port as number | undefined;
    const token = sess.token as string | undefined;

    if (!token) {
      throw new SessionError(
        "session.json is missing token; is the daemon running?"
      );
    }

    // On Unix the session contains socket_path; on Windows it contains rpc_port.
    const addr =
      socketPathVal || (rpcPort ? `127.0.0.1:${rpcPort}` : "");
    if (!addr) {
      throw new SessionError(
        "session.json is missing socket_path and rpc_port; " +
          "is the daemon running?"
      );
    }

    return new BrowserCLI(addr, token, timeout);
  }

  // ------------------------------------------------------------------
  // Low-level RPC
  // ------------------------------------------------------------------

  /**
   * Send an RPC request and return the parsed JSON response.
   */
  private _request(
    method: string,
    urlPath: string,
    body?: Record<string, unknown>
  ): Promise<any> {
    return new Promise((resolve, reject) => {
      const payload = body ? JSON.stringify(body) : "";

      const requestOptions: http.RequestOptions = this._useTcp
        ? {
            hostname: this._addr.split(":")[0],
            port: parseInt(this._addr.split(":")[1], 10),
            method,
            path: urlPath,
            headers: {
              Authorization: `Bearer ${this._token}`,
              "Content-Type": "application/json",
              "Content-Length": Buffer.byteLength(payload),
            },
            timeout: this._timeout,
          }
        : {
            socketPath: this._addr,
            method,
            path: urlPath,
            headers: {
              Authorization: `Bearer ${this._token}`,
              "Content-Type": "application/json",
              "Content-Length": Buffer.byteLength(payload),
            },
            timeout: this._timeout,
          };

      const req = http.request(
        requestOptions,
        (res) => {
          let data = "";
          res.setEncoding("utf-8");
          res.on("data", (chunk: string) => {
            data += chunk;
          });
          res.on("end", () => {
            const status = res.statusCode ?? 0;

            if (status === 401) {
              reject(
                new AuthenticationError(
                  "Daemon rejected the bearer token. " +
                    "The daemon may have restarted — try BrowserCLI.connect() again."
                )
              );
              return;
            }

            if (status >= 400) {
              let errorMsg = data.trim();
              try {
                const errorJson = JSON.parse(data);
                if (
                  errorJson &&
                  typeof errorJson === "object" &&
                  typeof errorJson.error === "string"
                ) {
                  errorMsg = errorJson.error;
                }
              } catch {
                // Use raw data as error message.
              }

              if (status === 400) {
                reject(new BadRequestError(errorMsg));
              } else if (status === 404) {
                reject(new NotFoundError(errorMsg));
              } else if (status >= 500) {
                reject(new ServerError(status, errorMsg));
              } else {
                reject(new RPCError(status, errorMsg));
              }
              return;
            }

            // Success.
            if (!data.trim()) {
              resolve({});
              return;
            }
            try {
              resolve(JSON.parse(data));
            } catch (err: unknown) {
              reject(
                new RPCError(
                  status,
                  `Daemon returned invalid JSON (HTTP ${status}): ${(err as Error).message}`
                )
              );
            }
          });
        }
      );

      req.on("error", (err: Error) => {
        reject(
          new ConnectionError(
            `Failed to communicate with daemon at ${this._addr}: ${err.message}`
          )
        );
      });

      req.on("timeout", () => {
        req.destroy();
        reject(
          new ConnectionError(
            `Request timed out after ${this._timeout}ms`
          )
        );
      });

      if (payload) {
        req.write(payload);
      }
      req.end();
    });
  }

  // ------------------------------------------------------------------
  // High-level API
  // ------------------------------------------------------------------

  /** Return daemon and browser status. */
  async status(): Promise<StatusResponse> {
    return this._request("GET", "/status");
  }

  /** Return RPC and schema version info. */
  async version(): Promise<VersionResponse> {
    return this._request("GET", "/version");
  }

  /**
   * Navigate the browser to a URL or path.
   *
   * @param url - Absolute URL (`http://...`) or path (`/page`).
   *   Paths are resolved relative to the daemon's serve directory.
   * @returns Object with `url` (final URL) and `title`.
   */
  async goto(url: string): Promise<GotoResponse> {
    if (typeof url !== "string") {
      throw new TypeError(`url must be a string, got ${typeof url}`);
    }
    return this._request("POST", "/goto", { url });
  }

  /**
   * Evaluate a JavaScript expression and return the result value.
   *
   * @param expression - JavaScript code to evaluate in the page context.
   */
  async eval(expression: string): Promise<unknown> {
    if (typeof expression !== "string" || !expression.trim()) {
      throw new TypeError("expression must be a non-empty string");
    }
    const resp: EvalResponse = await this._request("POST", "/eval", {
      expression,
    });
    return resp.value;
  }

  /** Reload the current page. Returns `true` on success. */
  async reload(): Promise<boolean> {
    const resp: ReloadResponse = await this._request("POST", "/reload");
    return resp.ok ?? false;
  }

  /**
   * Query a single DOM element and return its content.
   *
   * @param selector - CSS selector string.
   * @param mode - `"outer_html"` (default) or `"text"`.
   */
  async domQuery(
    selector: string,
    mode: string = "outer_html"
  ): Promise<string> {
    if (typeof selector !== "string" || !selector.trim()) {
      throw new TypeError("selector must be a non-empty string");
    }
    if (!DOM_MODES.has(mode as any)) {
      throw new TypeError(
        `mode must be one of [${[...DOM_MODES].sort().join(", ")}], got '${mode}'`
      );
    }
    const resp: DomResponse = await this._request("POST", "/dom", {
      selector,
      mode,
    });
    return resp.value ?? "";
  }

  /**
   * Query all matching DOM elements.
   *
   * @param selector - CSS selector string.
   * @param mode - `"outer_html"` (default) or `"text"`.
   */
  async domAll(
    selector: string,
    mode: string = "outer_html"
  ): Promise<string[]> {
    if (typeof selector !== "string" || !selector.trim()) {
      throw new TypeError("selector must be a non-empty string");
    }
    if (!DOM_MODES.has(mode as any)) {
      throw new TypeError(
        `mode must be one of [${[...DOM_MODES].sort().join(", ")}], got '${mode}'`
      );
    }
    const resp: DomAllResponse = await this._request("POST", "/dom/all", {
      selector,
      mode,
    });
    return resp.values ?? [];
  }

  /**
   * Get an attribute value from an element.
   *
   * @param selector - CSS selector for the target element.
   * @param name - Attribute name (e.g. `"href"`, `"class"`).
   * @returns Attribute value, or `null` if the attribute doesn't exist.
   */
  async domAttr(selector: string, name: string): Promise<string | null> {
    if (typeof selector !== "string" || !selector.trim()) {
      throw new TypeError("selector must be a non-empty string");
    }
    if (typeof name !== "string" || !name.trim()) {
      throw new TypeError("name must be a non-empty string");
    }
    const resp: DomAttrResponse = await this._request("POST", "/dom/attr", {
      selector,
      name,
    });
    return resp.value;
  }

  /**
   * Click an element.
   *
   * @param selector - CSS selector for the element to click.
   */
  async domClick(selector: string): Promise<boolean> {
    if (typeof selector !== "string" || !selector.trim()) {
      throw new TypeError("selector must be a non-empty string");
    }
    const resp: DomClickResponse = await this._request(
      "POST",
      "/dom/click",
      { selector }
    );
    return resp.ok ?? false;
  }

  /**
   * Type text into an input element.
   *
   * @param selector - CSS selector for the input element.
   * @param text - Text to type.
   * @param clear - If `true`, clear the field before typing.
   */
  async domType(
    selector: string,
    text: string,
    clear: boolean = false
  ): Promise<boolean> {
    if (typeof selector !== "string" || !selector.trim()) {
      throw new TypeError("selector must be a non-empty string");
    }
    if (typeof text !== "string") {
      throw new TypeError(`text must be a string, got ${typeof text}`);
    }
    const resp: DomTypeResponse = await this._request("POST", "/dom/type", {
      selector,
      text,
      clear,
    });
    return resp.ok ?? false;
  }

  /**
   * Wait for an element to reach a given state.
   *
   * @param selector - CSS selector for the target element.
   * @param state - `"visible"` (default), `"hidden"`, `"attached"`, or `"detached"`.
   * @param timeoutMs - Maximum wait time in milliseconds (default 10000).
   */
  async domWait(
    selector: string,
    state: string = "visible",
    timeoutMs: number = 10000
  ): Promise<boolean> {
    if (typeof selector !== "string" || !selector.trim()) {
      throw new TypeError("selector must be a non-empty string");
    }
    if (!WAIT_STATES.has(state as any)) {
      throw new TypeError(
        `state must be one of [${[...WAIT_STATES].sort().join(", ")}], got '${state}'`
      );
    }
    if (!Number.isInteger(timeoutMs) || timeoutMs <= 0) {
      throw new TypeError("timeoutMs must be a positive integer");
    }
    const resp: DomWaitResponse = await this._request("POST", "/dom/wait", {
      selector,
      state,
      timeout_ms: timeoutMs,
    });
    return resp.ok ?? false;
  }

  /**
   * Take a screenshot. Returns raw PNG bytes as a Buffer.
   *
   * @param selector - Optional CSS selector to screenshot a specific element.
   *   Empty string (default) captures the full page.
   * @param out - If given, the PNG is also written to this file path.
   */
  async screenshot(
    selector: string = "",
    out?: string
  ): Promise<Buffer> {
    if (typeof selector !== "string") {
      throw new TypeError(
        `selector must be a string, got ${typeof selector}`
      );
    }
    const resp: ScreenshotResponse = await this._request(
      "POST",
      "/screenshot",
      { selector, format: "png" }
    );
    const b64Data = resp.base64 ?? "";
    if (!b64Data) {
      return Buffer.alloc(0);
    }
    const buf = Buffer.from(b64Data, "base64");
    if (out) {
      fs.writeFileSync(out, buf);
    }
    return buf;
  }

  /**
   * Fetch console entries.
   *
   * @param level - Filter by level: `"log"`, `"warn"`, `"error"`, `"info"`, or `""` (all).
   * @param limit - Maximum number of entries (0 = unlimited).
   * @param clear - If `true`, drain entries after reading.
   */
  async console(
    level: string = "",
    limit: number = 0,
    clear: boolean = false
  ): Promise<ConsoleEntry[]> {
    if (!CONSOLE_LEVELS.has(level as any)) {
      throw new TypeError(
        `level must be one of [${[...CONSOLE_LEVELS].sort().join(", ")}], got '${level}'`
      );
    }
    if (!Number.isInteger(limit) || limit < 0) {
      throw new TypeError("limit must be a non-negative integer");
    }
    const resp: ConsoleResponse = await this._request("POST", "/console", {
      level,
      limit,
      clear,
    });
    return resp.entries ?? [];
  }

  /**
   * Fetch network log entries.
   *
   * @param limit - Maximum number of entries (0 = unlimited).
   * @param clear - If `true`, drain entries after reading.
   */
  async network(
    limit: number = 0,
    clear: boolean = false
  ): Promise<NetworkEntry[]> {
    if (!Number.isInteger(limit) || limit < 0) {
      throw new TypeError("limit must be a non-negative integer");
    }
    const resp: NetworkResponse = await this._request("POST", "/network", {
      limit,
      clear,
    });
    return resp.entries ?? [];
  }

  /** Return page performance metrics. */
  async perf(): Promise<PerfResponse> {
    return this._request("GET", "/perf");
  }

  /** Stop the daemon. Returns `true` on success. */
  async stop(): Promise<boolean> {
    const resp: StopResponse = await this._request("POST", "/stop");
    return resp.ok ?? false;
  }

  // ------------------------------------------------------------------
  // Plugins
  // ------------------------------------------------------------------

  /**
   * List all installed plugins and their templates, hooks, and endpoints.
   */
  async pluginList(): Promise<PluginInfo[]> {
    const resp: PluginListResponse = await this._request("GET", "/plugins");
    return resp.plugins ?? [];
  }

  /**
   * Call a custom plugin RPC endpoint.
   *
   * @param rpcPath - The endpoint path (must start with `/x/`).
   * @param body - Optional JSON request body for the handler.
   * @returns The parsed JSON response from the plugin handler.
   */
  async pluginRpc(
    rpcPath: string,
    body?: Record<string, unknown>
  ): Promise<unknown> {
    if (typeof rpcPath !== "string" || !rpcPath.startsWith("/x/")) {
      throw new TypeError(
        `rpcPath must be a string starting with '/x/', got '${rpcPath}'`
      );
    }
    return this._request("POST", rpcPath, body);
  }

  // ------------------------------------------------------------------
  // Representation
  // ------------------------------------------------------------------

  toString(): string {
    return `BrowserCLI(addr=${this._addr}, timeout=${this._timeout})`;
  }
}
