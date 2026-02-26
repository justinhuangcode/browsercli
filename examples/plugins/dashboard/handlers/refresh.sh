#!/bin/sh
# RPC handler: /x/dashboard/refresh
#
# Receives JSON on stdin (the request body), outputs JSON on stdout.
# Environment variables provided by browsercli:
#   BROWSERCLI_HTTP_PORT  — HTTP server port
#   BROWSERCLI_DIR        — Serve directory path
#   BROWSERCLI_BASE_URL   — Base URL of the HTTP server
#   BROWSERCLI_PLUGIN_NAME — "dashboard"

# Read stdin (request body, may be empty).
INPUT=$(cat)

# Generate simulated refresh data.
TIMESTAMP=$(date +%s)
REQUESTS=$((RANDOM % 1000))
ERRORS=$((RANDOM % 10))
LATENCY=$((RANDOM % 200))

cat <<EOF
{
  "ok": true,
  "timestamp": ${TIMESTAMP},
  "metrics": {
    "requests": ${REQUESTS},
    "errors": ${ERRORS},
    "avg_latency_ms": ${LATENCY}
  }
}
EOF
