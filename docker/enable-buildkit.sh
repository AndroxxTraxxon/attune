#!/bin/bash
# enable-buildkit.sh - Enable Docker BuildKit for faster Rust builds
# This script configures Docker to use BuildKit, which enables cache mounts
# for dramatically faster incremental builds

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

echo -e "${BLUE}=================================================="
echo "Docker BuildKit Configuration"
echo -e "==================================================${NC}\n"

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    print_error "Docker is not installed"
    exit 1
fi

print_success "Docker is installed"

# Check current BuildKit status
print_info "Checking current BuildKit configuration..."

if [ -n "$DOCKER_BUILDKIT" ]; then
    print_info "DOCKER_BUILDKIT environment variable is set to: $DOCKER_BUILDKIT"
else
    print_warning "DOCKER_BUILDKIT environment variable is not set"
fi

# Determine shell
SHELL_NAME=$(basename "$SHELL")
SHELL_RC=""

case "$SHELL_NAME" in
    bash)
        if [ -f "$HOME/.bashrc" ]; then
            SHELL_RC="$HOME/.bashrc"
        elif [ -f "$HOME/.bash_profile" ]; then
            SHELL_RC="$HOME/.bash_profile"
        fi
        ;;
    zsh)
        SHELL_RC="$HOME/.zshrc"
        ;;
    fish)
        SHELL_RC="$HOME/.config/fish/config.fish"
        ;;
    *)
        SHELL_RC="$HOME/.profile"
        ;;
esac

echo ""
print_info "Detected shell: $SHELL_NAME"
print_info "Shell configuration file: $SHELL_RC"

# Check if already configured
if [ -f "$SHELL_RC" ] && grep -q "DOCKER_BUILDKIT" "$SHELL_RC"; then
    echo ""
    print_success "BuildKit is already configured in $SHELL_RC"

    # Check if it's enabled
    if grep -q "export DOCKER_BUILDKIT=1" "$SHELL_RC"; then
        print_success "BuildKit is ENABLED"
    else
        print_warning "BuildKit configuration found but may not be enabled"
        print_info "Check your $SHELL_RC file"
    fi
else
    echo ""
    print_warning "BuildKit is not configured in your shell"

    read -p "Would you like to enable BuildKit globally? (y/n) " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        echo "" >> "$SHELL_RC"
        echo "# Enable Docker BuildKit for faster builds" >> "$SHELL_RC"
        echo "export DOCKER_BUILDKIT=1" >> "$SHELL_RC"
        echo "export COMPOSE_DOCKER_CLI_BUILD=1" >> "$SHELL_RC"

        print_success "BuildKit configuration added to $SHELL_RC"
        print_info "Run: source $SHELL_RC  (or restart your terminal)"
    fi
fi

# Check Docker daemon configuration
DOCKER_CONFIG="/etc/docker/daemon.json"
HAS_SUDO=false

if command -v sudo &> /dev/null; then
    HAS_SUDO=true
fi

echo ""
print_info "Checking Docker daemon configuration..."

if [ -f "$DOCKER_CONFIG" ]; then
    if $HAS_SUDO && sudo test -r "$DOCKER_CONFIG"; then
        if sudo grep -q "\"features\"" "$DOCKER_CONFIG" && sudo grep -q "\"buildkit\"" "$DOCKER_CONFIG"; then
            print_success "BuildKit appears to be configured in Docker daemon"
        else
            print_warning "BuildKit may not be configured in Docker daemon"
            print_info "This is optional - environment variables are sufficient"
        fi
    else
        print_warning "Cannot read $DOCKER_CONFIG (permission denied)"
        print_info "This is normal for non-root users"
    fi
else
    print_info "Docker daemon config not found at $DOCKER_CONFIG"
    print_info "This is normal - environment variables work fine"
fi

# Test BuildKit
echo ""
print_info "Testing BuildKit availability..."

# Create a minimal test Dockerfile
TEST_DIR=$(mktemp -d)
cat > "$TEST_DIR/Dockerfile" <<'EOF'
FROM alpine:latest
RUN --mount=type=cache,target=/tmp/cache echo "BuildKit works!" > /tmp/cache/test
RUN echo "Test complete"
EOF

if DOCKER_BUILDKIT=1 docker build -q "$TEST_DIR" > /dev/null 2>&1; then
    print_success "BuildKit is working correctly!"
    print_success "Cache mounts are supported"
else
    print_error "BuildKit test failed"
    print_info "Your Docker version may not support BuildKit"
    print_info "BuildKit requires Docker 18.09+ with experimental features enabled"
fi

# Cleanup
rm -rf "$TEST_DIR"

# Display usage information
echo ""
echo -e "${BLUE}=================================================="
echo "Usage Information"
echo -e "==================================================${NC}"
echo ""
echo "To use BuildKit with Attune:"
echo ""
echo "1. Build with docker compose (recommended):"
echo "   export DOCKER_BUILDKIT=1"
echo "   docker compose build"
echo ""
echo "2. Build individual service:"
echo "   DOCKER_BUILDKIT=1 docker build --build-arg SERVICE=api -f docker/Dockerfile.optimized -t attune-api ."
echo ""
echo "3. Use Makefile:"
echo "   export DOCKER_BUILDKIT=1"
echo "   make docker-build"
echo ""
echo -e "${GREEN}Benefits of BuildKit:${NC}"
echo "  • First build: ~5-6 minutes"
echo "  • Incremental builds: ~30-60 seconds (instead of 5+ minutes)"
echo "  • Caches: Cargo registry, git dependencies, compilation artifacts"
echo "  • Parallel builds and improved layer caching"
echo ""
echo -e "${YELLOW}Note:${NC} Cache persists between builds, potentially using 5-10GB disk space"
echo "      To clear cache: docker builder prune"
echo ""

# Check for current environment
if [ "$DOCKER_BUILDKIT" = "1" ]; then
    print_success "BuildKit is currently ENABLED in this shell session"
else
    print_warning "BuildKit is NOT enabled in the current shell session"
    print_info "Run: export DOCKER_BUILDKIT=1"
fi

echo ""
print_success "Configuration check complete!"
