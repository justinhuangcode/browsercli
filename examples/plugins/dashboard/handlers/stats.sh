#!/bin/sh
# RPC handler: /x/dashboard/stats
#
# Returns current system statistics as JSON.

UPTIME_SECS=$(awk '{print int($1)}' /proc/uptime 2>/dev/null || echo "0")
LOAD=$(cat /proc/loadavg 2>/dev/null | awk '{print $1}' || echo "0.00")

cat <<EOF
{
  "ok": true,
  "system": {
    "uptime_seconds": ${UPTIME_SECS},
    "load_average": "${LOAD}",
    "plugin": "${BROWSERCLI_PLUGIN_NAME:-dashboard}"
  }
}
EOF
