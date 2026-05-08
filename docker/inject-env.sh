#!/bin/sh
# inject-env.sh - Injects runtime environment variables into the Web UI
# This script runs at container startup to make environment variables available to the browser
#
# When served via nginx reverse proxy (default Docker setup), the web client
# uses relative paths: API calls go to /api/... and WebSocket to /ws/...
# Both are proxied by nginx to the respective backend services.

set -e

# Default: empty string means "use relative paths through nginx proxy"
API_URL="${API_URL:-}"
WS_URL="${WS_URL:-}"

# Create runtime configuration file
cat > /usr/share/nginx/html/config/runtime-config.js <<EOF
// Runtime configuration injected at container startup
// Empty values = use relative paths via nginx reverse proxy (recommended)
window.__ATTUNE_RUNTIME_CONFIG__ = {
  apiUrl: '${API_URL}',
  wsUrl: '${WS_URL}',
  environment: '${ENVIRONMENT:-production}'
};
EOF

echo "Runtime configuration injected:"
echo "  API_URL: ${API_URL:-(relative, via nginx proxy)}"
echo "  WS_URL: ${WS_URL:-(relative, via nginx proxy)}"
echo "  ENVIRONMENT: ${ENVIRONMENT:-production}"
