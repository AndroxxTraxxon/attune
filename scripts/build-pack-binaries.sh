#!/usr/bin/env bash
# Build pack binaries using Docker and extract them to ./packs/
#
# This script builds native pack binaries (sensors, etc.) in a Docker container
# with GLIBC compatibility and extracts them to the appropriate pack directories.
#
# Usage:
#   ./scripts/build-pack-binaries.sh
#
# The script will:
# 1. Build pack binaries in a Docker container with GLIBC 2.36 (Debian Bookworm)
# 2. Extract binaries to ./packs/core/sensors/
# 3. Make binaries executable
# 4. Clean up temporary container

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

# Configuration
IMAGE_NAME="attune-pack-builder"
CONTAINER_NAME="attune-pack-binaries-tmp"
DOCKERFILE="docker/Dockerfile.pack-binaries"

echo -e "${GREEN}Building pack binaries...${NC}"
echo "Project root: ${PROJECT_ROOT}"
echo "Dockerfile: ${DOCKERFILE}"
echo ""

# Navigate to project root
cd "${PROJECT_ROOT}"

# Check if Dockerfile exists
if [[ ! -f "${DOCKERFILE}" ]]; then
    echo -e "${RED}Error: ${DOCKERFILE} not found${NC}"
    exit 1
fi

# Build the Docker image
echo -e "${YELLOW}Step 1/4: Building Docker image...${NC}"
if DOCKER_BUILDKIT=1 docker build \
    -f "${DOCKERFILE}" \
    -t "${IMAGE_NAME}" \
    . ; then
    echo -e "${GREEN}✓ Image built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build image${NC}"
    exit 1
fi

# Create a temporary container from the image
echo -e "${YELLOW}Step 2/4: Creating temporary container...${NC}"
if docker create --name "${CONTAINER_NAME}" "${IMAGE_NAME}" ; then
    echo -e "${GREEN}✓ Container created${NC}"
else
    echo -e "${RED}✗ Failed to create container${NC}"
    exit 1
fi

# Extract binaries from the container
echo -e "${YELLOW}Step 3/4: Extracting pack binaries...${NC}"

# Create target directories
mkdir -p packs/core/sensors

# Copy timer sensor binary
if docker cp "${CONTAINER_NAME}:/pack-binaries/attune-core-timer-sensor" "packs/core/sensors/attune-core-timer-sensor" ; then
    echo -e "${GREEN}✓ Extracted attune-core-timer-sensor${NC}"
else
    echo -e "${RED}✗ Failed to extract timer sensor binary${NC}"
    docker rm "${CONTAINER_NAME}" 2>/dev/null || true
    exit 1
fi

# Make binaries executable
chmod +x packs/core/sensors/attune-core-timer-sensor

# Verify binaries
echo ""
echo -e "${YELLOW}Verifying binaries:${NC}"
file packs/core/sensors/attune-core-timer-sensor
ldd packs/core/sensors/attune-core-timer-sensor || echo "(ldd failed - binary may be static or require different environment)"
ls -lh packs/core/sensors/attune-core-timer-sensor

# Clean up temporary container
echo ""
echo -e "${YELLOW}Step 4/4: Cleaning up...${NC}"
if docker rm "${CONTAINER_NAME}" ; then
    echo -e "${GREEN}✓ Temporary container removed${NC}"
else
    echo -e "${YELLOW}⚠ Failed to remove temporary container (may already be removed)${NC}"
fi

# Summary
echo ""
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo -e "${GREEN}Pack binaries built successfully!${NC}"
echo -e "${GREEN}════════════════════════════════════════${NC}"
echo ""
echo "Binaries location:"
echo "  • packs/core/sensors/attune-core-timer-sensor"
echo ""
echo "These binaries are now ready to be used by the init-packs service"
echo "when starting docker-compose."
echo ""
echo "To use them:"
echo "  docker compose up -d"
echo ""
