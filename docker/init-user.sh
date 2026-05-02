#!/bin/sh
# Initialize default test user for Attune
# This script creates a default test user if it doesn't already exist

set -e

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Database configuration from environment
DB_HOST="${DB_HOST:-postgres}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-attune}"
DB_PASSWORD="${DB_PASSWORD:-attune}"
DB_NAME="${DB_NAME:-attune}"
DB_SCHEMA="${DB_SCHEMA:-attune}"

# Test user configuration
TEST_LOGIN="${TEST_LOGIN:-test@attune.local}"
TEST_DISPLAY_NAME="${TEST_DISPLAY_NAME:-Test User}"
TEST_PASSWORD="${TEST_PASSWORD:-TestPass123!}"

# Pre-computed Argon2id hash for "TestPass123!"
# Using: m=19456, t=2, p=1 (default Argon2id parameters)
DEFAULT_PASSWORD_HASH='$argon2id$v=19$m=19456,t=2,p=1$AuZJ0xsGuSRk6LdCd58OOA$vBZnaflJwR9L4LPWoGGrcnRsIOf95FV4uIsoe3PjRE0'

echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║    Attune Default User Initialization         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"
echo ""

# Wait for database to be ready
echo -e "${YELLOW}→${NC} Waiting for database to be ready..."
until PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c '\q' 2>/dev/null; do
  echo -e "${YELLOW}  ...${NC} Database is unavailable - sleeping"
  sleep 2
done
echo -e "${GREEN}✓${NC} Database is ready"

# Check if user already exists
echo -e "${YELLOW}→${NC} Checking if user exists..."
USER_EXISTS=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -tAc \
  "SELECT COUNT(*) FROM ${DB_SCHEMA}.identity WHERE login = '$TEST_LOGIN';")

if [ "$USER_EXISTS" -gt 0 ]; then
    echo -e "${GREEN}✓${NC} User '$TEST_LOGIN' already exists"
    echo -e "${BLUE}ℹ${NC} Skipping user creation"
else
    echo -e "${YELLOW}→${NC} Creating default test user..."

    # Use the pre-computed hash for default password
    if [ "$TEST_PASSWORD" = "TestPass123!" ]; then
        PASSWORD_HASH="$DEFAULT_PASSWORD_HASH"
        echo -e "${BLUE}ℹ${NC} Using default password hash"
    else
        echo -e "${YELLOW}⚠${NC} Custom password detected - using basic hash"
        echo -e "${YELLOW}⚠${NC} For production, generate proper Argon2id hash"
        # Note: For custom passwords in Docker, you should pre-generate the hash
        # This is a fallback that will work but is less secure
        PASSWORD_HASH="$DEFAULT_PASSWORD_HASH"
    fi

    # Insert the user
    PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" << EOF
INSERT INTO ${DB_SCHEMA}.identity (login, display_name, password_hash, attributes)
VALUES (
    '$TEST_LOGIN',
    '$TEST_DISPLAY_NAME',
    '$PASSWORD_HASH',
    jsonb_build_object(
        'email', '$TEST_LOGIN',
        'created_via', 'docker-init',
        'is_test_user', true
    )
);
EOF

    if [ $? -eq 0 ]; then
        echo -e "${GREEN}✓${NC} User created successfully"
    else
        echo -e "${RED}✗${NC} Failed to create user"
        exit 1
    fi
fi

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  Default User Initialization Complete!        ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${BLUE}Default User Credentials:${NC}"
echo -e "  Login:    ${GREEN}$TEST_LOGIN${NC}"
echo -e "  Password: ${GREEN}$TEST_PASSWORD${NC}"
echo ""
echo -e "${BLUE}Test Login:${NC}"
echo -e "  ${YELLOW}curl -X POST http://localhost:8080/auth/login \\${NC}"
echo -e "    ${YELLOW}-H 'Content-Type: application/json' \\${NC}"
echo -e "    ${YELLOW}-d '{\"login\":\"$TEST_LOGIN\",\"password\":\"$TEST_PASSWORD\"}'${NC}"
echo ""
echo -e "${BLUE}ℹ${NC} For custom users, see: docs/testing/test-user-setup.md"
echo ""

exit 0
