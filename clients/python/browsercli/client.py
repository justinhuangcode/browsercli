"""Core client that talks to the browsercli daemon over its RPC API.

On macOS/Linux the daemon listens on a Unix socket; on Windows it uses
TCP localhost.  The transport is chosen automatically based on the session
file contents.
"""

from __future__ import annotations

import base64
import http.client
import json
import logging
import os
import socket
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Union

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

logger = logging.getLogger("browsercli")

# Valid values for dom_query / dom_all ``mode`` parameter.
DOM_MODES = frozenset({"outer_html", "text"})

# Valid values for dom_wait ``state`` parameter.
WAIT_STATES = frozenset({"visible", "hidden", "attached", "detached"})

# Valid values for console ``level`` filter.
CONSOLE_LEVELS = frozenset({"", "log", "warn", "error", "info"})


class _UnixHTTPConnection(http.client.HTTPConnection):
    """HTTPConnection subclass that connects to a Unix domain socket."""

    def __init__(self, socket_path: str, timeout: float = 30.0) -> None:
        # host is unused but required by HTTPConnection
        super().__init__("localhost", timeout=timeout)
        self._socket_path = socket_path

    def connect(self) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.settimeout(self.timeout)
        self.sock.connect(self._socket_path)


class BrowserCLI:
    """Client for a running browsercli daemon.

    Usage::

        from browsercli import BrowserCLI

        ac = BrowserCLI.connect()       # reads ~/.browsercli/session.json
        print(ac.status())
        ac.goto("/")
        text = ac.dom_query("h1", mode="text")
        ac.stop()

    All methods are synchronous and block until the daemon responds.
    """

    def __init__(
        self,
        socket_path: str = "",
        token: str = "",
        timeout: float = 30.0,
        *,
        rpc_port: int = 0,
    ) -> None:
        if not socket_path and not rpc_port:
            raise ValueError("must provide either socket_path or rpc_port")
        if not token:
            raise ValueError("token must be a non-empty string")
        if timeout <= 0:
            raise ValueError("timeout must be positive")
        self._socket_path = socket_path
        self._rpc_port = rpc_port
        self._token = token
        self._timeout = timeout

    # ------------------------------------------------------------------
    # Factory
    # ------------------------------------------------------------------

    @classmethod
    def connect(
        cls,
        session_path: Optional[str] = None,
        timeout: float = 30.0,
    ) -> BrowserCLI:
        """Create a client by reading the daemon's session file.

        Args:
            session_path: Explicit path to ``session.json``.  Defaults to
                ``~/.browsercli/session.json`` on macOS/Linux, or
                ``%LOCALAPPDATA%\\browsercli\\session.json`` on Windows.
            timeout: Socket timeout in seconds (must be positive).

        Returns:
            A connected :class:`BrowserCLI` instance.

        Raises:
            SessionError: If the session file is missing, unreadable, or
                contains invalid data.
        """
        if session_path is None:
            if sys.platform == "win32":
                app_data = os.environ.get("LOCALAPPDATA", str(Path.home()))
                session_path = str(
                    Path(app_data) / "browsercli" / "session.json"
                )
            else:
                session_path = str(
                    Path.home() / ".browsercli" / "session.json"
                )

        try:
            with open(session_path) as f:
                sess = json.load(f)
        except FileNotFoundError:
            raise SessionError(
                f"Session file not found: {session_path} — "
                "is the daemon running? (browsercli start)"
            )
        except (json.JSONDecodeError, OSError) as exc:
            raise SessionError(
                f"Cannot read session file {session_path}: {exc}"
            ) from exc

        if not isinstance(sess, dict):
            raise SessionError(
                f"Session file {session_path} does not contain a JSON object"
            )

        socket_path_val = sess.get("socket_path", "")
        rpc_port_val = sess.get("rpc_port", 0)
        token = sess.get("token", "")
        if not token:
            raise SessionError(
                "session.json is missing token; is the daemon running?"
            )
        if not socket_path_val and not rpc_port_val:
            raise SessionError(
                "session.json is missing socket_path and rpc_port; "
                "is the daemon running?"
            )
        return cls(
            socket_path_val,
            token,
            timeout=timeout,
            rpc_port=rpc_port_val,
        )

    # ------------------------------------------------------------------
    # Low-level RPC
    # ------------------------------------------------------------------

    def _request(
        self,
        method: str,
        path: str,
        body: Optional[Dict[str, Any]] = None,
    ) -> Any:
        """Send an RPC request and return the parsed JSON response.

        Raises:
            ConnectionError: Socket connection failed.
            AuthenticationError: HTTP 401 (token rejected).
            BadRequestError: HTTP 400 (malformed request).
            NotFoundError: HTTP 404 (unknown endpoint or element).
            ServerError: HTTP 5xx (daemon internal error).
            RPCError: Any other non-2xx status.
        """
        logger.debug("RPC %s %s body=%s", method, path, body)

        try:
            if self._socket_path:
                conn = _UnixHTTPConnection(
                    self._socket_path, timeout=self._timeout
                )
            else:
                conn = http.client.HTTPConnection(
                    "127.0.0.1", port=self._rpc_port, timeout=self._timeout
                )
        except OSError as exc:
            addr_desc = self._socket_path or f"127.0.0.1:{self._rpc_port}"
            raise ConnectionError(
                f"Cannot create connection to {addr_desc}: {exc}"
            ) from exc

        try:
            headers = {
                "Authorization": f"Bearer {self._token}",
                "Content-Type": "application/json",
            }
            payload = json.dumps(body).encode() if body else b""

            try:
                conn.request(method, path, body=payload, headers=headers)
                resp = conn.getresponse()
            except (OSError, socket.timeout) as exc:
                addr_desc = self._socket_path or f"127.0.0.1:{self._rpc_port}"
                raise ConnectionError(
                    f"Failed to communicate with daemon at "
                    f"{addr_desc}: {exc}"
                ) from exc

            raw_data = resp.read().decode("utf-8", errors="replace")
            status = resp.status

            logger.debug("RPC response status=%d body=%s", status, raw_data[:200])

            if status == 401:
                raise AuthenticationError(
                    "Daemon rejected the bearer token. "
                    "The daemon may have restarted — try BrowserCLI.connect() again."
                )

            if status >= 400:
                # Try to extract the structured error message.
                error_msg = raw_data.strip()
                try:
                    error_json = json.loads(raw_data)
                    if isinstance(error_json, dict) and "error" in error_json:
                        error_msg = error_json["error"]
                except (json.JSONDecodeError, ValueError):
                    pass  # Use raw_data as-is

                if status == 400:
                    raise BadRequestError(error_msg)
                elif status == 404:
                    raise NotFoundError(error_msg)
                elif status >= 500:
                    raise ServerError(status, error_msg)
                else:
                    raise RPCError(status, error_msg)

            # Success: parse JSON.
            if not raw_data.strip():
                return {}
            try:
                return json.loads(raw_data)
            except json.JSONDecodeError as exc:
                raise RPCError(
                    status,
                    f"Daemon returned invalid JSON (HTTP {status}): {exc}",
                ) from exc
        finally:
            conn.close()

    # ------------------------------------------------------------------
    # High-level API
    # ------------------------------------------------------------------

    def status(self) -> Dict[str, Any]:
        """Return daemon and browser status.

        Response keys include ``running``, ``browser_alive``, ``pid``,
        ``dir``, ``http_addr``, ``http_port``, ``current_url``, ``title``,
        ``headless``, ``browser_pid``, ``devtools_port``, ``browser_bin``.
        """
        return self._request("GET", "/status")

    def version(self) -> Dict[str, Any]:
        """Return RPC and schema version info.

        Response keys: ``rpc_version``, ``schema_version``.
        """
        return self._request("GET", "/version")

    def goto(self, url: str) -> Dict[str, Any]:
        """Navigate the browser to *url* (path or full URL).

        Args:
            url: Absolute URL (``http://...``) or path (``/page``).
                Paths are resolved relative to the daemon's serve directory.

        Returns:
            Dict with ``url`` (final URL after navigation) and ``title``.
        """
        if not isinstance(url, str):
            raise TypeError(f"url must be a string, got {type(url).__name__}")
        return self._request("POST", "/goto", {"url": url})

    def eval(self, expression: str) -> Any:
        """Evaluate a JavaScript expression and return the result value.

        Args:
            expression: JavaScript code to evaluate in the page context.

        Returns:
            The evaluated result (any JSON-serializable type, or None).
        """
        if not isinstance(expression, str) or not expression.strip():
            raise ValueError("expression must be a non-empty string")
        resp = self._request("POST", "/eval", {"expression": expression})
        return resp.get("value")

    def reload(self) -> bool:
        """Reload the current page.

        Returns:
            True on success.
        """
        resp = self._request("POST", "/reload")
        return resp.get("ok", False)

    def dom_query(
        self,
        selector: str,
        mode: str = "outer_html",
    ) -> str:
        """Query a single DOM element and return its content.

        Args:
            selector: CSS selector string.
            mode: ``"outer_html"`` (default) or ``"text"``.

        Returns:
            The element content as a string.

        Raises:
            ValueError: If *mode* is not a recognized value.
        """
        if not isinstance(selector, str) or not selector.strip():
            raise ValueError("selector must be a non-empty string")
        if mode not in DOM_MODES:
            raise ValueError(
                f"mode must be one of {sorted(DOM_MODES)}, got {mode!r}"
            )
        resp = self._request(
            "POST", "/dom", {"selector": selector, "mode": mode}
        )
        return resp.get("value", "")

    def dom_all(
        self,
        selector: str,
        mode: str = "outer_html",
    ) -> List[str]:
        """Query all matching DOM elements.

        Args:
            selector: CSS selector string.
            mode: ``"outer_html"`` (default) or ``"text"``.

        Returns:
            List of content strings for each matching element.

        Raises:
            ValueError: If *mode* is not a recognized value.
        """
        if not isinstance(selector, str) or not selector.strip():
            raise ValueError("selector must be a non-empty string")
        if mode not in DOM_MODES:
            raise ValueError(
                f"mode must be one of {sorted(DOM_MODES)}, got {mode!r}"
            )
        resp = self._request(
            "POST", "/dom/all", {"selector": selector, "mode": mode}
        )
        return resp.get("values", [])

    def dom_attr(self, selector: str, name: str) -> Optional[str]:
        """Get an attribute value from an element.

        Args:
            selector: CSS selector for the target element.
            name: Attribute name (e.g. ``"href"``, ``"class"``).

        Returns:
            Attribute value, or ``None`` if the attribute doesn't exist.
        """
        if not isinstance(selector, str) or not selector.strip():
            raise ValueError("selector must be a non-empty string")
        if not isinstance(name, str) or not name.strip():
            raise ValueError("name must be a non-empty string")
        resp = self._request(
            "POST", "/dom/attr", {"selector": selector, "name": name}
        )
        return resp.get("value")

    def dom_click(self, selector: str) -> bool:
        """Click an element.

        Args:
            selector: CSS selector for the element to click.

        Returns:
            True on success.
        """
        if not isinstance(selector, str) or not selector.strip():
            raise ValueError("selector must be a non-empty string")
        resp = self._request(
            "POST", "/dom/click", {"selector": selector}
        )
        return resp.get("ok", False)

    def dom_type(
        self,
        selector: str,
        text: str,
        clear: bool = False,
    ) -> bool:
        """Type text into an input element.

        Args:
            selector: CSS selector for the input element.
            text: Text to type.
            clear: If True, clear the field before typing.

        Returns:
            True on success.
        """
        if not isinstance(selector, str) or not selector.strip():
            raise ValueError("selector must be a non-empty string")
        if not isinstance(text, str):
            raise TypeError(f"text must be a string, got {type(text).__name__}")
        resp = self._request(
            "POST",
            "/dom/type",
            {"selector": selector, "text": text, "clear": clear},
        )
        return resp.get("ok", False)

    def dom_wait(
        self,
        selector: str,
        state: str = "visible",
        timeout_ms: int = 10000,
    ) -> bool:
        """Wait for an element to reach a given state.

        Args:
            selector: CSS selector for the target element.
            state: ``"visible"`` (default), ``"hidden"``, ``"attached"``,
                or ``"detached"``.
            timeout_ms: Maximum wait time in milliseconds (default 10000).

        Returns:
            True if the element reached the desired state.

        Raises:
            ValueError: If *state* is not a recognized value.
        """
        if not isinstance(selector, str) or not selector.strip():
            raise ValueError("selector must be a non-empty string")
        if state not in WAIT_STATES:
            raise ValueError(
                f"state must be one of {sorted(WAIT_STATES)}, got {state!r}"
            )
        if not isinstance(timeout_ms, int) or timeout_ms <= 0:
            raise ValueError("timeout_ms must be a positive integer")
        resp = self._request(
            "POST",
            "/dom/wait",
            {
                "selector": selector,
                "state": state,
                "timeout_ms": timeout_ms,
            },
        )
        return resp.get("ok", False)

    def screenshot(
        self,
        selector: str = "",
        out: Optional[Union[str, Path]] = None,
    ) -> bytes:
        """Take a screenshot.  Returns raw PNG bytes.

        Args:
            selector: Optional CSS selector to screenshot a specific element.
                Empty string (default) captures the full page.
            out: If given, the PNG is also written to this file path.

        Returns:
            Raw PNG image bytes.
        """
        if not isinstance(selector, str):
            raise TypeError(f"selector must be a string, got {type(selector).__name__}")
        resp = self._request(
            "POST",
            "/screenshot",
            {"selector": selector, "format": "png"},
        )
        b64_data = resp.get("base64", "")
        if not b64_data:
            return b""
        raw = base64.b64decode(b64_data)
        if out:
            Path(out).write_bytes(raw)
        return raw

    def console(
        self,
        level: str = "",
        limit: int = 0,
        clear: bool = False,
    ) -> List[Dict[str, Any]]:
        """Fetch console entries.

        Args:
            level: Filter by level: ``"log"``, ``"warn"``, ``"error"``,
                ``"info"``, or ``""`` (all).
            limit: Maximum number of entries to return (0 = unlimited).
            clear: If True, drain (delete) entries after reading.

        Returns:
            List of dicts with keys ``level``, ``text``, ``timestamp``.

        Raises:
            ValueError: If *level* is not a recognized value.
        """
        if level not in CONSOLE_LEVELS:
            raise ValueError(
                f"level must be one of {sorted(CONSOLE_LEVELS)}, got {level!r}"
            )
        if not isinstance(limit, int) or limit < 0:
            raise ValueError("limit must be a non-negative integer")
        resp = self._request(
            "POST",
            "/console",
            {"level": level, "limit": limit, "clear": clear},
        )
        return resp.get("entries", [])

    def network(
        self,
        limit: int = 0,
        clear: bool = False,
    ) -> List[Dict[str, Any]]:
        """Fetch network log entries.

        Args:
            limit: Maximum number of entries to return (0 = unlimited).
            clear: If True, drain (delete) entries after reading.

        Returns:
            List of dicts with keys ``method``, ``url``, ``status``,
            ``resource_type``, ``mime_type``, ``size``, ``duration_ms``,
            ``timestamp``.
        """
        if not isinstance(limit, int) or limit < 0:
            raise ValueError("limit must be a non-negative integer")
        resp = self._request(
            "POST",
            "/network",
            {"limit": limit, "clear": clear},
        )
        return resp.get("entries", [])

    def perf(self) -> Dict[str, float]:
        """Return page performance metrics.

        Returns:
            Dict with ``dom_content_loaded_ms`` and ``load_event_ms``.
        """
        return self._request("GET", "/perf")

    def stop(self) -> bool:
        """Stop the daemon.

        Returns:
            True on success.
        """
        resp = self._request("POST", "/stop")
        return resp.get("ok", False)

    def plugin_list(self) -> List[Dict[str, Any]]:
        """List all installed plugins.

        Returns:
            List of dicts with keys ``name``, ``version``, ``description``,
            ``templates``, ``hooks``, ``rpc_endpoints``.
        """
        resp = self._request("GET", "/plugins")
        return resp.get("plugins", [])

    def plugin_rpc(
        self,
        rpc_path: str,
        body: Optional[Dict[str, Any]] = None,
    ) -> Any:
        """Call a custom plugin RPC endpoint.

        Args:
            rpc_path: The endpoint path (must start with ``/x/``).
            body: Optional JSON request body for the handler.

        Returns:
            The parsed JSON response from the plugin handler.

        Raises:
            ValueError: If *rpc_path* doesn't start with ``/x/``.
        """
        if not isinstance(rpc_path, str) or not rpc_path.startswith("/x/"):
            raise ValueError(
                f"rpc_path must be a string starting with '/x/', got {rpc_path!r}"
            )
        return self._request("POST", rpc_path, body)

    # ------------------------------------------------------------------
    # Context manager
    # ------------------------------------------------------------------

    def __enter__(self) -> BrowserCLI:
        return self

    def __exit__(self, *exc: Any) -> None:
        pass  # does NOT auto-stop; caller decides

    # ------------------------------------------------------------------
    # repr
    # ------------------------------------------------------------------

    def __repr__(self) -> str:
        addr = self._socket_path or f"127.0.0.1:{self._rpc_port}"
        return f"BrowserCLI(addr={addr!r}, timeout={self._timeout})"
