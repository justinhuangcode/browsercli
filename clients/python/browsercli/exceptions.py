"""Exception hierarchy for the browsercli Python client.

All public exceptions inherit from :class:`BrowserCLIError` so callers
can catch the whole family with a single ``except BrowserCLIError``.
"""

from __future__ import annotations


class BrowserCLIError(Exception):
    """Base exception for all browsercli client errors."""


class ConnectionError(BrowserCLIError):
    """The client could not connect to the daemon Unix socket.

    Common causes:
    - The daemon is not running (``browsercli start`` not called).
    - The socket file was deleted or has wrong permissions.
    - The session file points to a stale socket.
    """


class AuthenticationError(BrowserCLIError):
    """The daemon rejected the bearer token (HTTP 401).

    Common causes:
    - The daemon was restarted and generated a new token.
    - The session file is stale.  Re-read it with ``BrowserCLI.connect()``.
    """


class RPCError(BrowserCLIError):
    """The daemon returned an HTTP error with a JSON ``{"error": "..."}`` body.

    Attributes:
        status_code: HTTP status code (e.g. 400, 404, 500).
        error_message: Human-readable error message from the daemon.
    """

    def __init__(self, status_code: int, error_message: str) -> None:
        self.status_code = status_code
        self.error_message = error_message
        super().__init__(f"RPC error {status_code}: {error_message}")


class BadRequestError(RPCError):
    """The daemon returned HTTP 400 — the request body was malformed."""

    def __init__(self, error_message: str) -> None:
        super().__init__(400, error_message)


class NotFoundError(RPCError):
    """The daemon returned HTTP 404 — unknown endpoint or missing element."""

    def __init__(self, error_message: str) -> None:
        super().__init__(404, error_message)


class ServerError(RPCError):
    """The daemon returned HTTP 5xx — an internal error occurred."""

    def __init__(self, status_code: int, error_message: str) -> None:
        super().__init__(status_code, error_message)


class SessionError(BrowserCLIError):
    """The session file is missing, unreadable, or has invalid content."""
