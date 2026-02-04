#!/bin/sh
# inject-env.sh - Injects runtime environment variables into the Web UI
# This script runs at container startup to make environment variables available to the browser

set -e

# Default values
API_URL="${API_URL:-http://localhost:8080}"
WS_URL="${WS_URL:-ws://localhost:8081}"

# Create runtime configuration file
cat > /usr/share/nginx/html/config/runtime-config.js <<EOF
// Runtime configuration injected at container startup
window.ATTUNE_CONFIG = {
  apiUrl: '${API_URL}',
  wsUrl: '${WS_URL}',
  environment: '${ENVIRONMENT:-production}'
};
EOF

echo "Runtime configuration injected:"
echo "  API_URL: ${API_URL}"
echo "  WS_URL: ${WS_URL}"
echo "  ENVIRONMENT: ${ENVIRONMENT:-production}"
