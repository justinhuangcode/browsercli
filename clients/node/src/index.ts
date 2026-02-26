/**
 * browsercli Node.js client — public API surface.
 *
 * @example
 * ```ts
 * import { BrowserCLI } from "browsercli";
 *
 * const ac = BrowserCLI.connect();
 * console.log(await ac.status());
 * await ac.goto("/");
 * const text = await ac.domQuery("h1", "text");
 * await ac.stop();
 * ```
 */

export { BrowserCLI } from "./client.js";

export {
  BrowserCLIError,
  ConnectionError,
  AuthenticationError,
  SessionError,
  RPCError,
  BadRequestError,
  NotFoundError,
  ServerError,
} from "./errors.js";

export type {
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
} from "./types.js";

export { DOM_MODES, WAIT_STATES, CONSOLE_LEVELS } from "./constants.js";
export type { DomMode, WaitState, ConsoleLevel } from "./constants.js";

/** Client library version. */
export const VERSION = "0.4.0";
