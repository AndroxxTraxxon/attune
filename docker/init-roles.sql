-- Docker initialization script
-- Creates the svc_attune role needed by migrations
-- This runs before migrations via docker-compose

-- Create service role for the application
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'svc_attune') THEN
        CREATE ROLE svc_attune WITH LOGIN PASSWORD 'attune_service_password';
    END IF;
END
$$;

-- Create API role
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'attune_api') THEN
        CREATE ROLE attune_api WITH LOGIN PASSWORD 'attune_api_password';
    END IF;
END
$$;

-- Grant basic permissions
GRANT ALL PRIVILEGES ON DATABASE attune TO svc_attune;
GRANT ALL PRIVILEGES ON DATABASE attune TO attune_api;

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";
