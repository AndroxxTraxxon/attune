#!/bin/bash
# init-db.sh - Database initialization script for Docker
# This script runs migrations and sets up the initial database schema

set -e

echo "=================================================="
echo "Attune Database Initialization"
echo "=================================================="

# Wait for PostgreSQL to be ready
echo "Waiting for PostgreSQL to be ready..."
until pg_isready -h postgres -U attune -d attune > /dev/null 2>&1; do
  echo "  PostgreSQL is unavailable - sleeping"
  sleep 2
done

echo "✓ PostgreSQL is ready"

# Check if schema exists
SCHEMA_EXISTS=$(psql -h postgres -U attune -d attune -tAc "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'attune');")

if [ "$SCHEMA_EXISTS" = "f" ]; then
  echo "Creating attune schema..."
  psql -h postgres -U attune -d attune -c "CREATE SCHEMA IF NOT EXISTS attune;"
  echo "✓ Schema created"
else
  echo "✓ Schema already exists"
fi

# Set search path
echo "Setting search path..."
psql -h postgres -U attune -d attune -c "ALTER DATABASE attune SET search_path TO attune, public;"
echo "✓ Search path configured"

# Run migrations
echo "Running database migrations..."
cd /opt/attune
sqlx migrate run

echo "✓ Migrations complete"

# Check table count
TABLE_COUNT=$(psql -h postgres -U attune -d attune -tAc "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = 'attune';")
echo "✓ Database has ${TABLE_COUNT} tables"

# Load core pack if needed
if [ -f /opt/attune/scripts/load-core-pack.sh ]; then
  echo "Loading core pack..."
  /opt/attune/scripts/load-core-pack.sh || echo "⚠ Core pack load failed (may already exist)"
fi

echo "=================================================="
echo "Database initialization complete!"
echo "=================================================="
