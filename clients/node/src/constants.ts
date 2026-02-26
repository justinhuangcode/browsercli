/** Valid values for `domQuery` / `domAll` `mode` parameter. */
export const DOM_MODES = new Set(["outer_html", "text"] as const);
export type DomMode = "outer_html" | "text";

/** Valid values for `domWait` `state` parameter. */
export const WAIT_STATES = new Set([
  "visible",
  "hidden",
  "attached",
  "detached",
] as const);
export type WaitState = "visible" | "hidden" | "attached" | "detached";

/** Valid values for `console` `level` filter. */
export const CONSOLE_LEVELS = new Set([
  "",
  "log",
  "warn",
  "error",
  "info",
] as const);
export type ConsoleLevel = "" | "log" | "warn" | "error" | "info";
