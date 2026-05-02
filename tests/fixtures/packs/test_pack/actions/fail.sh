#!/bin/sh
# Failing Action for E2E Testing (Shell version)
# Always exits with non-zero exit code
echo '{"error": "Action intentionally failed"}' >&2
exit 1
