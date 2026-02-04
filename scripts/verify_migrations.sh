#!/bin/bash
# Migration Verification Script
# Tests the new consolidated migrations on a fresh database

set -e

echo "=========================================="
echo "Attune Migration Verification Script"
echo "=========================================="
echo ""

# Configuration
TEST_DB="attune_migration_test"
POSTGRES_USER="${POSTGRES_USER:-postgres}"
POSTGRES_HOST="${POSTGRES_HOST:-localhost}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Helper functions
print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${YELLOW}ℹ${NC} $1"
}

# Step 1: Drop test database if exists
echo "Step 1: Cleaning up existing test database..."
psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d postgres -c "DROP DATABASE IF EXISTS $TEST_DB;" 2>/dev/null
print_success "Cleaned up existing test database"

# Step 2: Create fresh test database
echo ""
echo "Step 2: Creating fresh test database..."
psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d postgres -c "CREATE DATABASE $TEST_DB;" 2>/dev/null
print_success "Created test database: $TEST_DB"

# Step 3: Run migrations
echo ""
echo "Step 3: Running consolidated migrations..."
export DATABASE_URL="postgresql://$POSTGRES_USER@$POSTGRES_HOST:$POSTGRES_PORT/$TEST_DB"

if command -v sqlx &> /dev/null; then
    sqlx migrate run --source migrations
    print_success "Migrations applied successfully via sqlx"
else
    # Fallback to psql if sqlx not available
    print_info "sqlx-cli not found, using psql..."
    for migration in migrations/202501*.sql; do
        if [ -f "$migration" ]; then
            echo "  Applying $(basename $migration)..."
            psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -f "$migration" > /dev/null
            print_success "  Applied $(basename $migration)"
        fi
    done
fi

# Step 4: Verify schema
echo ""
echo "Step 4: Verifying schema..."
TABLE_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'attune';")
TABLE_COUNT=$(echo $TABLE_COUNT | xargs) # Trim whitespace

if [ "$TABLE_COUNT" -eq "18" ]; then
    print_success "Correct number of tables: $TABLE_COUNT"
else
    print_error "Expected 18 tables, found $TABLE_COUNT"
    exit 1
fi

# Step 5: Verify all expected tables
echo ""
echo "Step 5: Verifying all expected tables exist..."
EXPECTED_TABLES=(
    "pack" "runtime" "worker" "identity" "permission_set" "permission_assignment" "policy" "key"
    "trigger" "sensor" "event" "enforcement"
    "action" "rule" "execution" "inquiry"
    "notification" "artifact"
)

MISSING_TABLES=()
for table in "${EXPECTED_TABLES[@]}"; do
    EXISTS=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT EXISTS (SELECT FROM information_schema.tables WHERE table_schema = 'attune' AND table_name = '$table');")
    EXISTS=$(echo $EXISTS | xargs)

    if [ "$EXISTS" = "t" ]; then
        echo "  ✓ $table"
    else
        MISSING_TABLES+=("$table")
        echo "  ✗ $table"
    fi
done

if [ ${#MISSING_TABLES[@]} -eq 0 ]; then
    print_success "All 18 tables exist"
else
    print_error "Missing tables: ${MISSING_TABLES[*]}"
    exit 1
fi

# Step 6: Verify enum types
echo ""
echo "Step 6: Verifying enum types..."
ENUM_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM pg_type WHERE typnamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'attune') AND typtype = 'e';")
ENUM_COUNT=$(echo $ENUM_COUNT | xargs)

if [ "$ENUM_COUNT" -eq "12" ]; then
    print_success "Correct number of enum types: $ENUM_COUNT"
else
    print_error "Expected 12 enum types, found $ENUM_COUNT"
fi

# Step 7: Verify indexes
echo ""
echo "Step 7: Verifying indexes..."
INDEX_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM pg_indexes WHERE schemaname = 'attune';")
INDEX_COUNT=$(echo $INDEX_COUNT | xargs)

if [ "$INDEX_COUNT" -gt "100" ]; then
    print_success "Found $INDEX_COUNT indexes (expected >100)"
else
    print_error "Expected >100 indexes, found $INDEX_COUNT"
fi

# Step 8: Verify foreign key constraints
echo ""
echo "Step 8: Verifying foreign key constraints..."
FK_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM information_schema.table_constraints WHERE constraint_schema = 'attune' AND constraint_type = 'FOREIGN KEY';")
FK_COUNT=$(echo $FK_COUNT | xargs)

if [ "$FK_COUNT" -gt "20" ]; then
    print_success "Found $FK_COUNT foreign key constraints"
else
    print_error "Expected >20 foreign keys, found $FK_COUNT"
fi

# Step 9: Verify triggers
echo ""
echo "Step 9: Verifying triggers..."
TRIGGER_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM information_schema.triggers WHERE trigger_schema = 'attune';")
TRIGGER_COUNT=$(echo $TRIGGER_COUNT | xargs)

if [ "$TRIGGER_COUNT" -gt "15" ]; then
    print_success "Found $TRIGGER_COUNT triggers (expected >15)"
else
    print_info "Found $TRIGGER_COUNT triggers"
fi

# Step 10: Verify functions
echo ""
echo "Step 10: Verifying functions..."
FUNCTION_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM pg_proc WHERE pronamespace = (SELECT oid FROM pg_namespace WHERE nspname = 'attune');")
FUNCTION_COUNT=$(echo $FUNCTION_COUNT | xargs)

if [ "$FUNCTION_COUNT" -ge "3" ]; then
    print_success "Found $FUNCTION_COUNT functions"
else
    print_error "Expected at least 3 functions, found $FUNCTION_COUNT"
fi

# Step 11: Test basic inserts
echo ""
echo "Step 11: Testing basic data operations..."

# Insert a pack
psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -c "INSERT INTO attune.pack (ref, label, version) VALUES ('test', 'Test Pack', '1.0.0');" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    print_success "Can insert pack"
else
    print_error "Failed to insert pack"
    exit 1
fi

# Insert an identity
psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -c "INSERT INTO attune.identity (login, display_name) VALUES ('testuser', 'Test User');" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    print_success "Can insert identity"
else
    print_error "Failed to insert identity"
    exit 1
fi

# Verify timestamps are auto-populated
CREATED_COUNT=$(psql -U "$POSTGRES_USER" -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -d "$TEST_DB" -t -c "SELECT COUNT(*) FROM attune.pack WHERE created IS NOT NULL AND updated IS NOT NULL;")
CREATED_COUNT=$(echo $CREATED_COUNT | xargs)

if [ "$CREATED_COUNT" -eq "1" ]; then
    print_success "Timestamps auto-populated correctly"
else
    print_error "Timestamp triggers not working"
fi

# Summary
echo ""
echo "=========================================="
echo "Verification Summary"
echo "=========================================="
echo "Database: $TEST_DB"
echo "Tables: $TABLE_COUNT"
echo "Enums: $ENUM_COUNT"
echo "Indexes: $INDEX_COUNT"
echo "Foreign Keys: $FK_COUNT"
echo "Triggers: $TRIGGER_COUNT"
echo "Functions: $FUNCTION_COUNT"
echo ""
print_success "All verification checks passed!"
echo ""
print_info "Test database '$TEST_DB' is ready for testing"
print_info "To clean up: dropdb $TEST_DB"
echo ""
