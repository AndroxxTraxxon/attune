#!/bin/bash
# Database Setup Script for Attune
# This script creates the database and runs migrations

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default values
DB_NAME="${ATTUNE_DB_NAME:-attune}"
DB_USER="${ATTUNE_DB_USER:-postgres}"
DB_HOST="${ATTUNE_DB_HOST:-localhost}"
DB_PORT="${ATTUNE_DB_PORT:-5432}"
DB_PASSWORD="${ATTUNE_DB_PASSWORD:-postgres}"

# Functions
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_postgres() {
    print_info "Checking PostgreSQL connection..."
    if ! command -v psql &> /dev/null; then
        print_error "psql command not found. Please install PostgreSQL client."
        exit 1
    fi

    if ! PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d postgres -c '\q' 2>/dev/null; then
        print_error "Cannot connect to PostgreSQL server at $DB_HOST:$DB_PORT"
        print_error "Please check your database connection settings."
        exit 1
    fi

    print_info "PostgreSQL connection successful!"
}

check_database_exists() {
    PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='$DB_NAME'" | grep -q 1
}

create_database() {
    if check_database_exists; then
        print_warn "Database '$DB_NAME' already exists."
        read -p "Do you want to drop and recreate it? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            print_info "Dropping database '$DB_NAME'..."
            PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d postgres -c "DROP DATABASE IF EXISTS $DB_NAME;"
        else
            print_info "Keeping existing database."
            return 0
        fi
    fi

    print_info "Creating database '$DB_NAME'..."
    PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d postgres -c "CREATE DATABASE $DB_NAME;"
    print_info "Database created successfully!"
}

run_migrations() {
    print_info "Running migrations..."

    export DATABASE_URL="postgresql://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME"

    # Check if sqlx-cli is installed
    if ! command -v sqlx &> /dev/null; then
        print_warn "sqlx-cli not found. Installing..."
        cargo install sqlx-cli --no-default-features --features postgres
    fi

    # Run migrations
    cd "$(dirname "$0")/.."

    if sqlx migrate run; then
        print_info "Migrations completed successfully!"
    else
        print_error "Migration failed!"
        exit 1
    fi
}

run_manual_migrations() {
    print_info "Running migrations manually with psql..."

    cd "$(dirname "$0")/.."

    for migration_file in migrations/*.sql; do
        if [ -f "$migration_file" ]; then
            print_info "Applying $(basename "$migration_file")..."
            if ! PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$migration_file"; then
                print_error "Failed to apply $(basename "$migration_file")"
                exit 1
            fi
        fi
    done

    print_info "All migrations applied successfully!"
}

verify_schema() {
    print_info "Verifying schema..."

    # Check if attune schema exists
    schema_exists=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -tAc "SELECT 1 FROM information_schema.schemata WHERE schema_name='attune'")

    if [ "$schema_exists" = "1" ]; then
        print_info "Schema 'attune' exists."

        # Count tables
        table_count=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -tAc "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='attune'")
        print_info "Found $table_count tables in attune schema."

        # List tables
        print_info "Tables:"
        PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "\dt attune.*"
    else
        print_error "Schema 'attune' not found!"
        exit 1
    fi
}

show_help() {
    cat << EOF
Attune Database Setup Script

Usage: $0 [OPTIONS]

Options:
    -h, --help          Show this help message
    -c, --create-only   Only create database (don't run migrations)
    -m, --migrate-only  Only run migrations (don't create database)
    -M, --manual        Run migrations manually with psql (without sqlx-cli)
    -v, --verify        Verify schema after setup

Environment Variables:
    ATTUNE_DB_NAME      Database name (default: attune)
    ATTUNE_DB_USER      Database user (default: postgres)
    ATTUNE_DB_HOST      Database host (default: localhost)
    ATTUNE_DB_PORT      Database port (default: 5432)
    ATTUNE_DB_PASSWORD  Database password (default: postgres)

Example:
    # Full setup
    $0

    # Create database only
    $0 --create-only

    # Run migrations only
    $0 --migrate-only

    # Use custom connection
    ATTUNE_DB_NAME=mydb ATTUNE_DB_PASSWORD=secret $0
EOF
}

# Main script
main() {
    local create_only=false
    local migrate_only=false
    local manual_migrations=false
    local verify=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_help
                exit 0
                ;;
            -c|--create-only)
                create_only=true
                shift
                ;;
            -m|--migrate-only)
                migrate_only=true
                shift
                ;;
            -M|--manual)
                manual_migrations=true
                shift
                ;;
            -v|--verify)
                verify=true
                shift
                ;;
            *)
                print_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done

    print_info "Attune Database Setup"
    print_info "====================="
    print_info "Database: $DB_NAME"
    print_info "Host: $DB_HOST:$DB_PORT"
    print_info "User: $DB_USER"
    echo

    check_postgres

    if [ "$migrate_only" = false ]; then
        create_database
    fi

    if [ "$create_only" = false ]; then
        if [ "$manual_migrations" = true ]; then
            run_manual_migrations
        else
            run_migrations
        fi
    fi

    if [ "$verify" = true ]; then
        verify_schema
    fi

    echo
    print_info "Database setup complete!"
    print_info "Connection string: postgresql://$DB_USER:***@$DB_HOST:$DB_PORT/$DB_NAME"
}

# Run main function
main "$@"
