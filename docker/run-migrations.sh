#!/bin/bash
# Migration script for Attune database
# Runs all SQL migration files in order

set -e

echo "=========================================="
echo "Attune Database Migration Runner"
echo "=========================================="
echo ""

# Database connection parameters
DB_HOST="${DB_HOST:-postgres}"
DB_PORT="${DB_PORT:-5432}"
DB_USER="${DB_USER:-attune}"
DB_PASSWORD="${DB_PASSWORD:-attune}"
DB_NAME="${DB_NAME:-attune}"

MIGRATIONS_DIR="${MIGRATIONS_DIR:-/migrations}"

# Export password for psql
export PGPASSWORD="$DB_PASSWORD"

# Color output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to wait for PostgreSQL to be ready
wait_for_postgres() {
    echo "Waiting for PostgreSQL to be ready..."
    local max_attempts=30
    local attempt=1

    while [ $attempt -le $max_attempts ]; do
        if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c '\q' 2>/dev/null; then
            echo -e "${GREEN}✓ PostgreSQL is ready${NC}"
            return 0
        fi

        echo "  Attempt $attempt/$max_attempts: PostgreSQL not ready yet..."
        sleep 2
        attempt=$((attempt + 1))
    done

    echo -e "${RED}✗ PostgreSQL failed to become ready after $max_attempts attempts${NC}"
    return 1
}

# Function to check if migrations table exists
setup_migrations_table() {
    echo "Setting up migrations tracking table..."

    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 <<-EOSQL
        CREATE TABLE IF NOT EXISTS _migrations (
            id SERIAL PRIMARY KEY,
            filename VARCHAR(255) UNIQUE NOT NULL,
            applied_at TIMESTAMP DEFAULT NOW()
        );
EOSQL

    echo -e "${GREEN}✓ Migrations table ready${NC}"
}

# Function to check if a migration has been applied
is_migration_applied() {
    local filename=$1
    local count=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c \
        "SELECT COUNT(*) FROM _migrations WHERE filename = '$filename';" | tr -d ' ')
    [ "$count" -gt 0 ]
}

# Function to mark migration as applied
mark_migration_applied() {
    local filename=$1
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c \
        "INSERT INTO _migrations (filename) VALUES ('$filename');" > /dev/null
}

# Function to run a migration file
run_migration() {
    local filepath=$1
    local filename=$(basename "$filepath")

    if is_migration_applied "$filename"; then
        echo -e "${YELLOW}⊘ Skipping $filename (already applied)${NC}"
        return 0
    fi

    echo -e "${GREEN}→ Applying $filename...${NC}"

    # Run migration in a transaction with detailed error reporting
    if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 \
        -c "BEGIN;" \
        -f "$filepath" \
        -c "COMMIT;" > /tmp/migration_output.log 2>&1; then
        mark_migration_applied "$filename"
        echo -e "${GREEN}✓ Applied $filename${NC}"
        return 0
    else
        echo -e "${RED}✗ Failed to apply $filename${NC}"
        echo ""
        echo "Error details:"
        cat /tmp/migration_output.log
        echo ""
        echo "Migration rolled back due to error."
        return 1
    fi
}

# Function to initialize Docker-specific roles and extensions
init_docker_roles() {
    echo "Initializing Docker roles and extensions..."

    if [ -f "/docker/init-roles.sql" ]; then
        if psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -f "/docker/init-roles.sql" > /dev/null 2>&1; then
            echo -e "${GREEN}✓ Docker roles initialized${NC}"
            return 0
        else
            echo -e "${YELLOW}⚠ Warning: Could not initialize Docker roles (may already exist)${NC}"
            return 0
        fi
    else
        echo -e "${YELLOW}⚠ No Docker init script found, skipping${NC}"
        return 0
    fi
}

# Main migration process
main() {
    echo "Configuration:"
    echo "  Database: $DB_HOST:$DB_PORT/$DB_NAME"
    echo "  User: $DB_USER"
    echo "  Migrations directory: $MIGRATIONS_DIR"
    echo ""

    # Wait for database
    wait_for_postgres || exit 1

    # Initialize Docker-specific roles
    init_docker_roles || exit 1

    # Setup migrations tracking
    setup_migrations_table || exit 1

    echo ""
    echo "Running migrations..."
    echo "----------------------------------------"

    # Find and sort migration files
    local migration_count=0
    local applied_count=0
    local skipped_count=0

    # Process migrations in sorted order
    for migration_file in $(find "$MIGRATIONS_DIR" -name "*.sql" -type f | sort); do
        migration_count=$((migration_count + 1))

        if is_migration_applied "$(basename "$migration_file")"; then
            skipped_count=$((skipped_count + 1))
            run_migration "$migration_file"
        else
            if run_migration "$migration_file"; then
                applied_count=$((applied_count + 1))
            else
                echo -e "${RED}Migration failed!${NC}"
                exit 1
            fi
        fi
    done

    echo "----------------------------------------"
    echo ""
    echo "Migration Summary:"
    echo "  Total migrations: $migration_count"
    echo "  Newly applied: $applied_count"
    echo "  Already applied: $skipped_count"
    echo ""

    if [ $applied_count -gt 0 ]; then
        echo -e "${GREEN}✓ All migrations applied successfully!${NC}"
    else
        echo -e "${GREEN}✓ Database is up to date (no new migrations)${NC}"
    fi
}

# Run main function
main
