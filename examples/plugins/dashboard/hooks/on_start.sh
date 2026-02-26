#!/bin/sh
# Lifecycle hook: on_daemon_start
#
# Runs when the browsercli daemon starts up.
# Fire-and-forget — output goes to daemon logs.

echo "[dashboard plugin] Daemon started"
echo "  HTTP port: ${BROWSERCLI_HTTP_PORT:-unknown}"
echo "  Serve dir: ${BROWSERCLI_DIR:-unknown}"
echo "  Base URL:  ${BROWSERCLI_BASE_URL:-unknown}"
