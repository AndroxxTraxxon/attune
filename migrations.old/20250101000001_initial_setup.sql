-- Migration: Initial Setup
-- Description: Creates the attune schema, enums, and shared database functions
-- Version: 20250101000001

-- ============================================================================
-- SCHEMA AND ROLE SETUP
-- ============================================================================

-- Create the attune schema
-- NOTE: For tests, the test schema is created separately. For production, uncomment below:
-- CREATE SCHEMA IF NOT EXISTS attune;

-- Set search path (now set via connection pool configuration)

-- Create service role for the application
-- NOTE: Commented out for tests, uncomment for production:
-- DO $$
-- BEGIN
--     IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'svc_attune') THEN
--         CREATE ROLE svc_attune WITH LOGIN PASSWORD 'attune_service_password';
--     END IF;
-- END
-- $$;

-- Grant usage on schema
-- NOTE: Commented out for tests, uncomment for production:
-- GRANT USAGE ON SCHEMA attune TO svc_attune;
-- GRANT CREATE ON SCHEMA attune TO svc_attune;

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- COMMENT ON SCHEMA attune IS 'Attune automation platform schema';

-- ============================================================================
-- ENUM TYPES
-- ============================================================================

-- RuntimeType enum
DO $$ BEGIN
    CREATE TYPE runtime_type_enum AS ENUM (
        'action',
        'sensor'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE runtime_type_enum IS 'Type of runtime environment';

-- WorkerType enum
DO $$ BEGIN
    CREATE TYPE worker_type_enum AS ENUM (
        'local',
        'remote',
        'container'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE worker_type_enum IS 'Type of worker deployment';

-- WorkerStatus enum
DO $$ BEGIN
    CREATE TYPE worker_status_enum AS ENUM (
        'active',
        'inactive',
        'busy',
        'error'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE worker_status_enum IS 'Worker operational status';

-- EnforcementStatus enum
DO $$ BEGIN
    CREATE TYPE enforcement_status_enum AS ENUM (
        'created',
        'processed',
        'disabled'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE enforcement_status_enum IS 'Enforcement processing status';

-- EnforcementCondition enum
DO $$ BEGIN
    CREATE TYPE enforcement_condition_enum AS ENUM (
        'any',
        'all'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE enforcement_condition_enum IS 'Logical operator for conditions (OR/AND)';

-- ExecutionStatus enum
DO $$ BEGIN
    CREATE TYPE execution_status_enum AS ENUM (
        'requested',
        'scheduling',
        'scheduled',
        'running',
        'completed',
        'failed',
        'canceling',
        'cancelled',
        'timeout',
        'abandoned'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE execution_status_enum IS 'Execution lifecycle status';

-- InquiryStatus enum
DO $$ BEGIN
    CREATE TYPE inquiry_status_enum AS ENUM (
        'pending',
        'responded',
        'timeout',
        'cancelled'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE inquiry_status_enum IS 'Inquiry lifecycle status';

-- PolicyMethod enum
DO $$ BEGIN
    CREATE TYPE policy_method_enum AS ENUM (
        'cancel',
        'enqueue'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE policy_method_enum IS 'Policy enforcement method';

-- OwnerType enum
DO $$ BEGIN
    CREATE TYPE owner_type_enum AS ENUM (
        'system',
        'identity',
        'pack',
        'action',
        'sensor'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE owner_type_enum IS 'Type of resource owner';

-- NotificationState enum
DO $$ BEGIN
    CREATE TYPE notification_status_enum AS ENUM (
        'created',
        'queued',
        'processing',
        'error'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE notification_status_enum IS 'Notification processing state';

-- ArtifactType enum
DO $$ BEGIN
    CREATE TYPE artifact_type_enum AS ENUM (
        'file_binary',
        'file_datatable',
        'file_image',
        'file_text',
        'other',
        'progress',
        'url'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE artifact_type_enum IS 'Type of artifact';

-- RetentionPolicyType enum
DO $$ BEGIN
    CREATE TYPE artifact_retention_enum AS ENUM (
        'versions',
        'days',
        'hours',
        'minutes'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

COMMENT ON TYPE artifact_retention_enum IS 'Type of retention policy';

-- ============================================================================
-- SHARED FUNCTIONS
-- ============================================================================

-- Function to automatically update the 'updated' timestamp
CREATE OR REPLACE FUNCTION update_updated_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION update_updated_column() IS 'Automatically updates the updated timestamp on row modification';
