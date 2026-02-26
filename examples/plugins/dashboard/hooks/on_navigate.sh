#!/bin/sh
# Lifecycle hook: on_navigate
#
# Runs when the browser navigates to a new URL.
# BROWSERCLI_URL is set to the new URL.

echo "[dashboard plugin] Navigation: ${BROWSERCLI_URL:-unknown}"
