/**
 * Exception hierarchy for the browsercli Node.js client.
 *
 * All public errors inherit from {@link BrowserCLIError} so callers
 * can catch the whole family with a single `catch (e) { if (e instanceof BrowserCLIError) }`.
 */

/** Base error for all browsercli client errors. */
export class BrowserCLIError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BrowserCLIError";
  }
}

/**
 * The client could not connect to the daemon Unix socket.
 *
 * Common causes:
 * - The daemon is not running (`browsercli start` not called).
 * - The socket file was deleted or has wrong permissions.
 * - The session file points to a stale socket.
 */
export class ConnectionError extends BrowserCLIError {
  constructor(message: string) {
    super(message);
    this.name = "ConnectionError";
  }
}

/**
 * The daemon rejected the bearer token (HTTP 401).
 *
 * Common causes:
 * - The daemon was restarted and generated a new token.
 * - The session file is stale.  Re-read it with `BrowserCLI.connect()`.
 */
export class AuthenticationError extends BrowserCLIError {
  constructor(message: string) {
    super(message);
    this.name = "AuthenticationError";
  }
}

/**
 * The daemon returned an HTTP error with a JSON `{"error": "..."}` body.
 *
 * Properties:
 * - `statusCode` — HTTP status code (e.g. 400, 404, 500).
 * - `errorMessage` — Human-readable error from the daemon.
 */
export class RPCError extends BrowserCLIError {
  public readonly statusCode: number;
  public readonly errorMessage: string;

  constructor(statusCode: number, errorMessage: string) {
    super(`RPC error ${statusCode}: ${errorMessage}`);
    this.name = "RPCError";
    this.statusCode = statusCode;
    this.errorMessage = errorMessage;
  }
}

/** The daemon returned HTTP 400 — the request body was malformed. */
export class BadRequestError extends RPCError {
  constructor(errorMessage: string) {
    super(400, errorMessage);
    this.name = "BadRequestError";
  }
}

/** The daemon returned HTTP 404 — unknown endpoint or missing element. */
export class NotFoundError extends RPCError {
  constructor(errorMessage: string) {
    super(404, errorMessage);
    this.name = "NotFoundError";
  }
}

/** The daemon returned HTTP 5xx — an internal error occurred. */
export class ServerError extends RPCError {
  constructor(statusCode: number, errorMessage: string) {
    super(statusCode, errorMessage);
    this.name = "ServerError";
  }
}

/** The session file is missing, unreadable, or has invalid content. */
export class SessionError extends BrowserCLIError {
  constructor(message: string) {
    super(message);
    this.name = "SessionError";
  }
}
