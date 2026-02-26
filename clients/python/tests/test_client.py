"""Comprehensive tests for the browsercli Python client.

Tests are grouped into:
- Session file parsing (no server)
- Parameter validation (no server)
- Contract tests with a mock server (Unix socket or TCP on Windows)
- TCP transport contract tests
"""

from __future__ import annotations

import base64
import http.server
import json
import os
import socket
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from typing import Any, Dict, Optional, Tuple

from browsercli.client import (
    BrowserCLI,
    CONSOLE_LEVELS,
    DOM_MODES,
    WAIT_STATES,
)
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

# Unix-only imports — used conditionally.
if sys.platform != "win32":
    import socketserver
    from browsercli.client import _UnixHTTPConnection


# ======================================================================
# Mock Unix Socket Server
# ======================================================================

# Pre-canned responses that match the Rust daemon's RPC output (server.rs).
MOCK_TOKEN = "deadbeef01234567deadbeef01234567"

MOCK_RESPONSES: Dict[str, Dict[str, Any]] = {
    "GET /version": {
        "status": 200,
        "body": {"rpc_version": 1, "schema_version": 1},
    },
    "GET /status": {
        "status": 200,
        "body": {
            "running": True,
            "browser_alive": True,
            "pid": 12345,
            "dir": "/tmp/serve",
            "http_addr": "127.0.0.1",
            "http_port": 8080,
            "current_url": "http://127.0.0.1:8080/",
            "title": "Test Page",
            "headless": True,
            "browser_pid": 12346,
            "devtools_port": 9222,
            "browser_bin": "/usr/bin/chromium",
        },
    },
    "POST /goto": {
        "status": 200,
        "body": {"url": "http://127.0.0.1:8080/about", "title": "About"},
    },
    "POST /eval": {
        "status": 200,
        "body": {"value": 42},
    },
    "POST /reload": {
        "status": 200,
        "body": {"ok": True},
    },
    "POST /dom": {
        "status": 200,
        "body": {"selector": "h1", "mode": "text", "value": "Hello"},
    },
    "POST /dom/all": {
        "status": 200,
        "body": {"selector": "p", "mode": "text", "values": ["a", "b", "c"]},
    },
    "POST /dom/attr": {
        "status": 200,
        "body": {"selector": "a", "name": "href", "value": "/about"},
    },
    "POST /dom/click": {
        "status": 200,
        "body": {"ok": True},
    },
    "POST /dom/type": {
        "status": 200,
        "body": {"ok": True},
    },
    "POST /dom/wait": {
        "status": 200,
        "body": {"ok": True, "state": "visible"},
    },
    "POST /screenshot": {
        "status": 200,
        "body": {
            "format": "png",
            "base64": base64.b64encode(b"\x89PNG\r\n\x1a\nfake").decode(),
        },
    },
    "POST /console": {
        "status": 200,
        "body": {
            "entries": [
                {"level": "log", "text": "hello", "timestamp": 1000},
                {"level": "error", "text": "boom", "timestamp": 2000},
            ]
        },
    },
    "POST /network": {
        "status": 200,
        "body": {
            "entries": [
                {
                    "method": "GET",
                    "url": "http://127.0.0.1:8080/",
                    "status": 200,
                    "resource_type": "Document",
                    "mime_type": "text/html",
                    "size": 1024,
                    "duration_ms": 50,
                    "timestamp": 3000,
                },
            ]
        },
    },
    "GET /perf": {
        "status": 200,
        "body": {"dom_content_loaded_ms": 150.5, "load_event_ms": 300.2},
    },
    "POST /stop": {
        "status": 200,
        "body": {"ok": True},
    },
    "GET /plugins": {
        "status": 200,
        "body": {
            "plugins": [
                {
                    "name": "test-plugin",
                    "version": "1.0.0",
                    "description": "A test plugin",
                    "templates": ["dashboard"],
                    "hooks": ["on_daemon_start"],
                    "rpc_endpoints": ["/x/test-plugin/hello"],
                },
            ]
        },
    },
    "POST /x/test-plugin/hello": {
        "status": 200,
        "body": {"plugin": "test-plugin", "message": "hello!"},
    },
}


class _MockHandler(http.server.BaseHTTPRequestHandler):
    """Handler that emulates the browsercli Rust daemon RPC server."""

    # Set by the test class to override specific behaviors.
    override_response: Optional[Tuple[int, Any]] = None
    require_auth: bool = True

    def log_message(self, format: str, *args: Any) -> None:
        pass  # Silence request logging during tests.

    def _handle(self) -> None:
        # Auth check — matches server.rs behavior.
        if self.require_auth:
            auth = self.headers.get("Authorization", "")
            if auth != f"Bearer {MOCK_TOKEN}":
                # The Rust daemon returns plain text "unauthorized" on 401.
                self.send_response(401)
                self.send_header("Content-Type", "text/plain")
                self.end_headers()
                self.wfile.write(b"unauthorized")
                return

        # Read body for POST.
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length) if content_length > 0 else b""

        # Check for override.
        if self.override_response is not None:
            status, resp_body = self.override_response
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(resp_body).encode())
            return

        key = f"{self.command} {self.path}"
        if key in MOCK_RESPONSES:
            resp = MOCK_RESPONSES[key]
            self.send_response(resp["status"])
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(resp["body"]).encode())
        else:
            # 404 — matches server.rs: `(404, json!({"error": "not found"}))`
            self.send_response(404)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"error": "not found"}).encode())

    do_GET = _handle
    do_POST = _handle


# Unix-only mock server — not available on Windows.
if sys.platform != "win32":

    class _UnixHTTPServer(socketserver.UnixStreamServer):
        """HTTP server that listens on a Unix socket."""

        allow_reuse_address = True

        def __init__(self, socket_path: str, handler_class: type) -> None:
            self._socket_path = socket_path
            super().__init__(socket_path, handler_class)

        def get_request(self) -> Tuple[socket.socket, Any]:
            client, addr = self.socket.accept()
            return client, addr

    class MockServer:
        """Context manager that runs a mock daemon on a temporary Unix socket."""

        def __init__(self) -> None:
            self.tmpdir = tempfile.mkdtemp()
            self.socket_path = os.path.join(self.tmpdir, "test.sock")
            self._server: Optional[_UnixHTTPServer] = None
            self._thread: Optional[threading.Thread] = None

        def start(self) -> None:
            self._server = _UnixHTTPServer(self.socket_path, _MockHandler)
            self._thread = threading.Thread(
                target=self._server.serve_forever, daemon=True
            )
            self._thread.start()

        def stop(self) -> None:
            if self._server:
                self._server.shutdown()
                self._server.server_close()
            try:
                os.unlink(self.socket_path)
            except OSError:
                pass
            try:
                os.rmdir(self.tmpdir)
            except OSError:
                pass

        def client(self, **kwargs: Any) -> BrowserCLI:
            """Create an BrowserCLI client connected to this mock server."""
            return BrowserCLI(
                self.socket_path, MOCK_TOKEN, **kwargs
            )

        def __enter__(self) -> MockServer:
            self.start()
            return self

        def __exit__(self, *exc: Any) -> None:
            self.stop()


# ======================================================================
# Test: Session file parsing
# ======================================================================

class TestConnectFactory(unittest.TestCase):
    """Tests for BrowserCLI.connect() session file parsing."""

    def test_connect_reads_session(self) -> None:
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump(
                {"socket_path": "/tmp/test.sock", "token": "abc123"}, f
            )
            f.flush()
            path = f.name
        try:
            ac = BrowserCLI.connect(session_path=path)
            self.assertEqual(ac._socket_path, "/tmp/test.sock")
            self.assertEqual(ac._token, "abc123")
        finally:
            os.unlink(path)

    def test_connect_missing_file_raises_session_error(self) -> None:
        with self.assertRaises(SessionError) as ctx:
            BrowserCLI.connect(
                session_path="/tmp/nonexistent-session-12345.json"
            )
        self.assertIn("not found", str(ctx.exception))
        self.assertIn("daemon running", str(ctx.exception))

    def test_connect_empty_token_raises_session_error(self) -> None:
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"socket_path": "/tmp/test.sock", "token": ""}, f)
            f.flush()
            path = f.name
        try:
            with self.assertRaises(SessionError):
                BrowserCLI.connect(session_path=path)
        finally:
            os.unlink(path)

    def test_connect_missing_socket_path_and_rpc_port_raises_session_error(self) -> None:
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"token": "abc"}, f)
            f.flush()
            path = f.name
        try:
            with self.assertRaises(SessionError):
                BrowserCLI.connect(session_path=path)
        finally:
            os.unlink(path)

    def test_connect_reads_rpc_port_session(self) -> None:
        """Session file with rpc_port (Windows format) instead of socket_path."""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump({"rpc_port": 12345, "token": "abc123"}, f)
            f.flush()
            path = f.name
        try:
            ac = BrowserCLI.connect(session_path=path)
            self.assertEqual(ac._rpc_port, 12345)
            self.assertEqual(ac._token, "abc123")
            self.assertEqual(ac._socket_path, "")
        finally:
            os.unlink(path)

    def test_connect_invalid_json_raises_session_error(self) -> None:
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            f.write("not valid json {{{")
            f.flush()
            path = f.name
        try:
            with self.assertRaises(SessionError):
                BrowserCLI.connect(session_path=path)
        finally:
            os.unlink(path)

    def test_connect_json_array_raises_session_error(self) -> None:
        """session.json must be a JSON object, not an array."""
        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".json", delete=False
        ) as f:
            json.dump([1, 2, 3], f)
            f.flush()
            path = f.name
        try:
            with self.assertRaises(SessionError):
                BrowserCLI.connect(session_path=path)
        finally:
            os.unlink(path)

    def test_session_error_is_browsercli_error(self) -> None:
        """SessionError should be catchable as BrowserCLIError."""
        with self.assertRaises(BrowserCLIError):
            BrowserCLI.connect(
                session_path="/tmp/nonexistent-session-12345.json"
            )


# ======================================================================
# Test: Constructor validation
# ======================================================================

class TestConstructorValidation(unittest.TestCase):
    def test_no_address_raises(self) -> None:
        """Must provide either socket_path or rpc_port."""
        with self.assertRaises(ValueError):
            BrowserCLI("", "token")

    def test_empty_token_raises(self) -> None:
        with self.assertRaises(ValueError):
            BrowserCLI("/tmp/test.sock", "")

    def test_negative_timeout_raises(self) -> None:
        with self.assertRaises(ValueError):
            BrowserCLI("/tmp/test.sock", "token", timeout=-1)

    def test_zero_timeout_raises(self) -> None:
        with self.assertRaises(ValueError):
            BrowserCLI("/tmp/test.sock", "token", timeout=0)

    def test_rpc_port_constructor(self) -> None:
        """Can construct with rpc_port instead of socket_path."""
        ac = BrowserCLI(token="tok", rpc_port=9999)
        self.assertEqual(ac._rpc_port, 9999)
        self.assertEqual(ac._socket_path, "")


# ======================================================================
# Test: Parameter validation (client-side, no server needed)
# ======================================================================

class TestParameterValidation(unittest.TestCase):
    """Verify client-side parameter validation before any RPC call."""

    def setUp(self) -> None:
        # Client with a fake socket path; we never actually connect.
        self.ac = BrowserCLI("/tmp/fake.sock", "tok")

    def test_goto_non_string_raises_type_error(self) -> None:
        with self.assertRaises(TypeError):
            self.ac.goto(123)  # type: ignore

    def test_eval_empty_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.eval("")

    def test_eval_whitespace_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.eval("   ")

    def test_dom_query_empty_selector_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_query("")

    def test_dom_query_invalid_mode_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_query("h1", mode="innerHTML")

    def test_dom_all_invalid_mode_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_all("p", mode="bad")

    def test_dom_attr_empty_name_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_attr("a", "")

    def test_dom_click_empty_selector_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_click("")

    def test_dom_type_non_string_text_raises(self) -> None:
        with self.assertRaises(TypeError):
            self.ac.dom_type("input", 123)  # type: ignore

    def test_dom_type_empty_selector_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_type("", "text")

    def test_dom_wait_invalid_state_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_wait("div", state="gone")

    def test_dom_wait_negative_timeout_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_wait("div", timeout_ms=-1)

    def test_dom_wait_zero_timeout_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.dom_wait("div", timeout_ms=0)

    def test_console_invalid_level_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.console(level="debug")

    def test_console_negative_limit_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.console(limit=-1)

    def test_network_negative_limit_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.network(limit=-1)

    def test_screenshot_non_string_selector_raises(self) -> None:
        with self.assertRaises(TypeError):
            self.ac.screenshot(selector=123)  # type: ignore

    def test_dom_modes_constant(self) -> None:
        self.assertEqual(DOM_MODES, {"outer_html", "text"})

    def test_wait_states_constant(self) -> None:
        self.assertEqual(WAIT_STATES, {"visible", "hidden", "attached", "detached"})

    def test_console_levels_constant(self) -> None:
        self.assertEqual(CONSOLE_LEVELS, {"", "log", "warn", "error", "info"})

    def test_plugin_rpc_path_without_x_prefix_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.plugin_rpc("/status")

    def test_plugin_rpc_non_string_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.plugin_rpc(123)  # type: ignore

    def test_plugin_rpc_empty_path_raises(self) -> None:
        with self.assertRaises(ValueError):
            self.ac.plugin_rpc("")


# ======================================================================
# Test: Context manager & repr
# ======================================================================

class TestContextManagerAndRepr(unittest.TestCase):
    def test_context_manager_returns_self(self) -> None:
        ac = BrowserCLI("/tmp/fake.sock", "tok")
        with ac as ctx:
            self.assertIs(ctx, ac)

    def test_repr_unix(self) -> None:
        ac = BrowserCLI("/tmp/fake.sock", "tok", timeout=5.0)
        r = repr(ac)
        self.assertIn("/tmp/fake.sock", r)
        self.assertIn("5.0", r)
        # Token should NOT appear in repr for security.
        self.assertNotIn("tok", r)

    def test_repr_tcp(self) -> None:
        ac = BrowserCLI(token="tok", timeout=5.0, rpc_port=9999)
        r = repr(ac)
        self.assertIn("127.0.0.1:9999", r)
        self.assertNotIn("tok", r)


# ======================================================================
# Test: Exception hierarchy
# ======================================================================

class TestExceptionHierarchy(unittest.TestCase):
    def test_all_inherit_from_browsercli_error(self) -> None:
        for cls in (
            ConnectionError,
            AuthenticationError,
            RPCError,
            BadRequestError,
            NotFoundError,
            ServerError,
            SessionError,
        ):
            self.assertTrue(
                issubclass(cls, BrowserCLIError),
                f"{cls.__name__} should inherit BrowserCLIError",
            )

    def test_bad_request_is_rpc_error(self) -> None:
        err = BadRequestError("bad")
        self.assertIsInstance(err, RPCError)
        self.assertEqual(err.status_code, 400)

    def test_not_found_is_rpc_error(self) -> None:
        err = NotFoundError("missing")
        self.assertIsInstance(err, RPCError)
        self.assertEqual(err.status_code, 404)

    def test_server_error_is_rpc_error(self) -> None:
        err = ServerError(500, "internal")
        self.assertIsInstance(err, RPCError)
        self.assertEqual(err.status_code, 500)

    def test_rpc_error_attributes(self) -> None:
        err = RPCError(422, "unprocessable")
        self.assertEqual(err.status_code, 422)
        self.assertEqual(err.error_message, "unprocessable")
        self.assertIn("422", str(err))
        self.assertIn("unprocessable", str(err))


# ======================================================================
# Test: Contract tests with mock server
# ======================================================================

@unittest.skipIf(sys.platform == "win32", "Unix socket mock server not available on Windows")
class TestContractWithMockServer(unittest.TestCase):
    """End-to-end contract tests using a mock Unix socket server.

    These verify that the Python client produces correct HTTP requests
    and correctly parses responses that match the Rust daemon's format.
    """

    @classmethod
    def setUpClass(cls) -> None:
        cls.mock = MockServer()
        cls.mock.start()
        cls.ac = cls.mock.client()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.mock.stop()

    # --- status ---
    def test_status(self) -> None:
        result = self.ac.status()
        self.assertTrue(result["running"])
        self.assertTrue(result["browser_alive"])
        self.assertEqual(result["pid"], 12345)
        self.assertEqual(result["dir"], "/tmp/serve")
        self.assertEqual(result["http_port"], 8080)
        self.assertEqual(result["title"], "Test Page")

    # --- version ---
    def test_version(self) -> None:
        result = self.ac.version()
        self.assertEqual(result["rpc_version"], 1)
        self.assertEqual(result["schema_version"], 1)

    # --- goto ---
    def test_goto(self) -> None:
        result = self.ac.goto("/about")
        self.assertEqual(result["url"], "http://127.0.0.1:8080/about")
        self.assertEqual(result["title"], "About")

    # --- eval ---
    def test_eval(self) -> None:
        result = self.ac.eval("1 + 1")
        self.assertEqual(result, 42)

    # --- reload ---
    def test_reload(self) -> None:
        self.assertTrue(self.ac.reload())

    # --- dom_query ---
    def test_dom_query(self) -> None:
        result = self.ac.dom_query("h1", mode="text")
        self.assertEqual(result, "Hello")

    # --- dom_all ---
    def test_dom_all(self) -> None:
        result = self.ac.dom_all("p", mode="text")
        self.assertEqual(result, ["a", "b", "c"])

    # --- dom_attr ---
    def test_dom_attr(self) -> None:
        result = self.ac.dom_attr("a", "href")
        self.assertEqual(result, "/about")

    # --- dom_click ---
    def test_dom_click(self) -> None:
        self.assertTrue(self.ac.dom_click("#btn"))

    # --- dom_type ---
    def test_dom_type(self) -> None:
        self.assertTrue(self.ac.dom_type("#input", "hello"))

    # --- dom_wait ---
    def test_dom_wait(self) -> None:
        self.assertTrue(self.ac.dom_wait("#el"))

    # --- screenshot ---
    def test_screenshot_returns_bytes(self) -> None:
        data = self.ac.screenshot()
        self.assertIsInstance(data, bytes)
        self.assertTrue(data.startswith(b"\x89PNG"))

    def test_screenshot_writes_file(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            path = f.name
        try:
            data = self.ac.screenshot(out=path)
            self.assertTrue(Path(path).exists())
            self.assertEqual(Path(path).read_bytes(), data)
        finally:
            os.unlink(path)

    # --- console ---
    def test_console(self) -> None:
        entries = self.ac.console()
        self.assertEqual(len(entries), 2)
        self.assertEqual(entries[0]["level"], "log")
        self.assertEqual(entries[0]["text"], "hello")
        self.assertEqual(entries[1]["level"], "error")

    # --- network ---
    def test_network(self) -> None:
        entries = self.ac.network()
        self.assertEqual(len(entries), 1)
        e = entries[0]
        self.assertEqual(e["method"], "GET")
        self.assertEqual(e["status"], 200)
        self.assertEqual(e["resource_type"], "Document")
        self.assertEqual(e["size"], 1024)
        self.assertEqual(e["duration_ms"], 50)

    # --- perf ---
    def test_perf(self) -> None:
        result = self.ac.perf()
        self.assertAlmostEqual(result["dom_content_loaded_ms"], 150.5)
        self.assertAlmostEqual(result["load_event_ms"], 300.2)

    # --- stop ---
    def test_stop(self) -> None:
        # We call stop but the mock server stays alive (no real shutdown).
        self.assertTrue(self.ac.stop())

    # --- plugin_list ---
    def test_plugin_list(self) -> None:
        plugins = self.ac.plugin_list()
        self.assertEqual(len(plugins), 1)
        self.assertEqual(plugins[0]["name"], "test-plugin")
        self.assertEqual(plugins[0]["version"], "1.0.0")
        self.assertEqual(plugins[0]["description"], "A test plugin")
        self.assertEqual(plugins[0]["templates"], ["dashboard"])
        self.assertEqual(plugins[0]["hooks"], ["on_daemon_start"])
        self.assertEqual(plugins[0]["rpc_endpoints"], ["/x/test-plugin/hello"])

    # --- plugin_rpc ---
    def test_plugin_rpc(self) -> None:
        result = self.ac.plugin_rpc("/x/test-plugin/hello")
        self.assertEqual(result["plugin"], "test-plugin")
        self.assertEqual(result["message"], "hello!")

    def test_plugin_rpc_with_body(self) -> None:
        result = self.ac.plugin_rpc("/x/test-plugin/hello", {"key": "value"})
        self.assertEqual(result["plugin"], "test-plugin")


# ======================================================================
# Test: Error handling with mock server
# ======================================================================

@unittest.skipIf(sys.platform == "win32", "Unix socket mock server not available on Windows")
class TestErrorHandling(unittest.TestCase):
    """Verify correct exception types for different error scenarios."""

    def test_auth_error_returns_authentication_error(self) -> None:
        """HTTP 401 with plain-text body 'unauthorized'."""
        with MockServer() as mock:
            # Wrong token.
            ac = BrowserCLI(mock.socket_path, "wrong_token")
            with self.assertRaises(AuthenticationError) as ctx:
                ac.status()
            self.assertIsInstance(ctx.exception, BrowserCLIError)

    def test_404_returns_not_found_error(self) -> None:
        """Unknown endpoint returns NotFoundError."""
        with MockServer() as mock:
            ac = mock.client()
            with self.assertRaises(NotFoundError) as ctx:
                ac._request("GET", "/nonexistent")
            self.assertEqual(ctx.exception.status_code, 404)
            self.assertEqual(ctx.exception.error_message, "not found")

    def test_400_returns_bad_request_error(self) -> None:
        """Malformed request body returns BadRequestError."""
        with MockServer() as mock:
            _MockHandler.override_response = (
                400,
                {"error": "missing field: selector"},
            )
            try:
                ac = mock.client()
                with self.assertRaises(BadRequestError) as ctx:
                    ac._request("POST", "/dom", {"bad": True})
                self.assertEqual(ctx.exception.status_code, 400)
                self.assertIn("selector", ctx.exception.error_message)
            finally:
                _MockHandler.override_response = None

    def test_500_returns_server_error(self) -> None:
        """Internal server error returns ServerError."""
        with MockServer() as mock:
            _MockHandler.override_response = (
                500,
                {"error": "CDP command timed out"},
            )
            try:
                ac = mock.client()
                with self.assertRaises(ServerError) as ctx:
                    ac._request("POST", "/eval", {"expression": "x"})
                self.assertEqual(ctx.exception.status_code, 500)
                self.assertIn("CDP", ctx.exception.error_message)
            finally:
                _MockHandler.override_response = None

    def test_connection_error_on_missing_socket(self) -> None:
        """Connecting to a non-existent socket raises ConnectionError."""
        ac = BrowserCLI("/tmp/nonexistent-sock-12345.sock", "tok")
        with self.assertRaises(ConnectionError):
            ac.status()

    def test_all_rpc_errors_are_browsercli_error(self) -> None:
        """All error types should be catchable with BrowserCLIError."""
        with MockServer() as mock:
            ac = BrowserCLI(mock.socket_path, "wrong_token")
            with self.assertRaises(BrowserCLIError):
                ac.status()

    def test_non_json_error_body_handled(self) -> None:
        """If the server returns non-JSON error body, we still get RPCError."""
        with MockServer() as mock:
            # Override to return plain text on error.
            _MockHandler.override_response = (500, "plain text error")
            try:
                ac = mock.client()
                with self.assertRaises(ServerError):
                    ac._request("GET", "/status")
            finally:
                _MockHandler.override_response = None


# ======================================================================
# Test: _UnixHTTPConnection
# ======================================================================

@unittest.skipIf(sys.platform == "win32", "_UnixHTTPConnection not available on Windows")
class TestUnixHTTPConnection(unittest.TestCase):
    def test_socket_path_stored(self) -> None:
        conn = _UnixHTTPConnection("/tmp/test.sock")
        self.assertEqual(conn._socket_path, "/tmp/test.sock")

    def test_custom_timeout(self) -> None:
        conn = _UnixHTTPConnection("/tmp/test.sock", timeout=5.0)
        self.assertEqual(conn.timeout, 5.0)


# ======================================================================
# Test: TCP transport contract tests
# ======================================================================

class _TcpMockServer:
    """Context manager that runs a mock daemon on TCP localhost."""

    def __init__(self) -> None:
        self._server: Optional[http.server.HTTPServer] = None
        self._thread: Optional[threading.Thread] = None
        self.port: int = 0

    def start(self) -> None:
        self._server = http.server.HTTPServer(
            ("127.0.0.1", 0), _MockHandler
        )
        self.port = self._server.server_address[1]
        self._thread = threading.Thread(
            target=self._server.serve_forever, daemon=True
        )
        self._thread.start()

    def stop(self) -> None:
        if self._server:
            self._server.shutdown()
            self._server.server_close()

    def client(self, **kwargs: Any) -> BrowserCLI:
        """Create an BrowserCLI client connected via TCP."""
        return BrowserCLI(
            token=MOCK_TOKEN, rpc_port=self.port, **kwargs
        )

    def __enter__(self) -> _TcpMockServer:
        self.start()
        return self

    def __exit__(self, *exc: Any) -> None:
        self.stop()


class TestTcpContractTests(unittest.TestCase):
    """Contract tests over TCP transport (Windows-compatible)."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.tcp_mock = _TcpMockServer()
        cls.tcp_mock.start()
        cls.ac = cls.tcp_mock.client()

    @classmethod
    def tearDownClass(cls) -> None:
        cls.tcp_mock.stop()

    def test_status_tcp(self) -> None:
        result = self.ac.status()
        self.assertTrue(result["running"])
        self.assertEqual(result["pid"], 12345)

    def test_goto_tcp(self) -> None:
        result = self.ac.goto("/about")
        self.assertEqual(result["url"], "http://127.0.0.1:8080/about")
        self.assertEqual(result["title"], "About")

    def test_eval_tcp(self) -> None:
        result = self.ac.eval("1 + 1")
        self.assertEqual(result, 42)

    def test_screenshot_tcp(self) -> None:
        data = self.ac.screenshot()
        self.assertIsInstance(data, bytes)
        self.assertTrue(data.startswith(b"\x89PNG"))

    def test_stop_tcp(self) -> None:
        self.assertTrue(self.ac.stop())

    def test_plugin_list_tcp(self) -> None:
        plugins = self.ac.plugin_list()
        self.assertEqual(len(plugins), 1)
        self.assertEqual(plugins[0]["name"], "test-plugin")

    def test_plugin_rpc_tcp(self) -> None:
        result = self.ac.plugin_rpc("/x/test-plugin/hello")
        self.assertEqual(result["plugin"], "test-plugin")
        self.assertEqual(result["message"], "hello!")

    def test_auth_error_tcp(self) -> None:
        ac = BrowserCLI(token="wrong_token", rpc_port=self.tcp_mock.port)
        with self.assertRaises(AuthenticationError):
            ac.status()

    def test_connection_error_tcp(self) -> None:
        """Connecting to unreachable TCP port raises ConnectionError."""
        ac = BrowserCLI(token="tok", rpc_port=1)
        with self.assertRaises(ConnectionError):
            ac.status()


if __name__ == "__main__":
    unittest.main()
