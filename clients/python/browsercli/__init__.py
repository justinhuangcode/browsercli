"""browsercli Python client — zero-dependency wrapper for the Unix socket RPC API."""

from browsercli.client import BrowserCLI
from browsercli.exceptions import (
    BrowserCLIError,
    AuthenticationError,
    BadRequestError,
    ConnectionError,
    NotFoundError,
    RPCError,
    ServerError,
    SessionError,
)

__all__ = [
    "BrowserCLI",
    "BrowserCLIError",
    "AuthenticationError",
    "BadRequestError",
    "ConnectionError",
    "NotFoundError",
    "RPCError",
    "ServerError",
    "SessionError",
]
__version__ = "0.4.0"
