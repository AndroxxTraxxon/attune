#!/bin/bash
# Check that all dependencies use workspace = true
# This ensures consistent dependency versions across the workspace

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "Checking workspace dependency compliance..."
echo ""

ERRORS=0
WARNINGS=0

# List of allowed exceptions (crate-specific dependencies that don't need workspace versions)
ALLOWED_EXCEPTIONS=(
    # Executor-specific
    "tera"
    "serde_yaml"

    # API-specific
    "jsonwebtoken"
    "hmac"
    "sha1"
    "hex"
    "utoipa"
    "utoipa-swagger-ui"
    "argon2"
    "rand"

    # CLI-specific
    "comfy-table"
    "dialoguer"
    "indicatif"
    "dirs"
    "urlencoding"
    "colored"

    # Sensor-specific
    "cron"

    # Worker-specific
    "hostname"

    # Common-specific
    "async-recursion"

    # Dev/test dependencies (crate-specific)
    "mockito"
    "wiremock"
    "criterion"
    "assert_cmd"
    "predicates"
    "tokio-test"
)

# Function to check if dependency is in allowed exceptions
is_allowed_exception() {
    local dep="$1"
    for exception in "${ALLOWED_EXCEPTIONS[@]}"; do
        if [[ "$dep" == "$exception" ]]; then
            return 0
        fi
    done
    return 1
}

# Check each crate's Cargo.toml
for crate in crates/*/Cargo.toml; do
    crate_name=$(basename $(dirname "$crate"))

    # Find dependencies that specify version directly (not using workspace)
    # Pattern: dep_name = "version" OR dep_name = { version = "..." } without workspace = true
    # Only look in [dependencies], [dev-dependencies], and [build-dependencies] sections
    violations=$(awk '
        /^\[/ { in_deps=0 }
        /^\[(dependencies|dev-dependencies|build-dependencies)\]/ { in_deps=1; next }
        in_deps && /^[a-z][a-z0-9_-]+ = / && !/workspace = true/ && !/path = / && /(= "|version = ")/ {
            match($0, /^[a-z][a-z0-9_-]+/);
            print substr($0, RSTART, RLENGTH)
        }
    ' "$crate" || true)

    if [ -n "$violations" ]; then
        has_real_violation=false

        while IFS= read -r dep; do
            dep=$(echo "$dep" | xargs) # trim whitespace

            if [ -n "$dep" ]; then
                if is_allowed_exception "$dep"; then
                    # Skip allowed exceptions
                    continue
                else
                    if [ "$has_real_violation" = false ]; then
                        echo -e "${YELLOW}Checking $crate_name...${NC}"
                        has_real_violation=true
                    fi

                    # Show the actual line
                    line=$(grep "^$dep = " "$crate")
                    echo -e "  ${RED}✗${NC} $dep"
                    echo -e "    ${RED}$line${NC}"
                    ERRORS=$((ERRORS + 1))
                fi
            fi
        done <<< "$violations"

        if [ "$has_real_violation" = true ]; then
            echo ""
        fi
    fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

if [ $ERRORS -gt 0 ]; then
    echo -e "${RED}✗ Found $ERRORS dependency version violation(s)${NC}"
    echo ""
    echo "All dependencies should use 'workspace = true' unless they are:"
    echo "  1. Crate-specific dependencies not used elsewhere"
    echo "  2. Listed in the allowed exceptions"
    echo ""
    echo "To fix:"
    echo "  1. Add the dependency to [workspace.dependencies] in Cargo.toml"
    echo "  2. Update the crate to use: dep_name = { workspace = true }"
    echo ""
    echo "Or add to ALLOWED_EXCEPTIONS in this script if it's crate-specific."
    exit 1
else
    echo -e "${GREEN}✓ All crates use workspace dependencies correctly${NC}"
    echo ""
    echo "Allowed exceptions: ${#ALLOWED_EXCEPTIONS[@]} crate-specific dependencies"
fi
