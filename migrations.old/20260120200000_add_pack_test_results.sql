-- Migration: Add Pack Test Results Tracking
-- Created: 2026-01-20
-- Description: Add tables and views for tracking pack test execution results

-- Pack test execution tracking table
CREATE TABLE IF NOT EXISTS pack_test_execution (
    id BIGSERIAL PRIMARY KEY,
    pack_id BIGINT NOT NULL REFERENCES pack(id) ON DELETE CASCADE,
    pack_version VARCHAR(50) NOT NULL,
    execution_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    trigger_reason VARCHAR(50) NOT NULL, -- 'install', 'update', 'manual', 'validation'
    total_tests INT NOT NULL,
    passed INT NOT NULL,
    failed INT NOT NULL,
    skipped INT NOT NULL,
    pass_rate DECIMAL(5,4) NOT NULL, -- 0.0000 to 1.0000
    duration_ms BIGINT NOT NULL,
    result JSONB NOT NULL, -- Full test result structure
    created TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT valid_test_counts CHECK (total_tests >= 0 AND passed >= 0 AND failed >= 0 AND skipped >= 0),
    CONSTRAINT valid_pass_rate CHECK (pass_rate >= 0.0 AND pass_rate <= 1.0),
    CONSTRAINT valid_trigger_reason CHECK (trigger_reason IN ('install', 'update', 'manual', 'validation'))
);

-- Indexes for efficient queries
CREATE INDEX idx_pack_test_execution_pack_id ON pack_test_execution(pack_id);
CREATE INDEX idx_pack_test_execution_time ON pack_test_execution(execution_time DESC);
CREATE INDEX idx_pack_test_execution_pass_rate ON pack_test_execution(pass_rate);
CREATE INDEX idx_pack_test_execution_trigger ON pack_test_execution(trigger_reason);

-- Comments for documentation
COMMENT ON TABLE pack_test_execution IS 'Tracks pack test execution results for validation and auditing';
COMMENT ON COLUMN pack_test_execution.pack_id IS 'Reference to the pack being tested';
COMMENT ON COLUMN pack_test_execution.pack_version IS 'Version of the pack at test time';
COMMENT ON COLUMN pack_test_execution.trigger_reason IS 'What triggered the test: install, update, manual, validation';
COMMENT ON COLUMN pack_test_execution.pass_rate IS 'Percentage of tests passed (0.0 to 1.0)';
COMMENT ON COLUMN pack_test_execution.result IS 'Full JSON structure with detailed test results';

-- Pack test result summary view (all test executions with pack info)
CREATE OR REPLACE VIEW pack_test_summary AS
SELECT
    p.id AS pack_id,
    p.ref AS pack_ref,
    p.label AS pack_label,
    pte.id AS test_execution_id,
    pte.pack_version,
    pte.execution_time AS test_time,
    pte.trigger_reason,
    pte.total_tests,
    pte.passed,
    pte.failed,
    pte.skipped,
    pte.pass_rate,
    pte.duration_ms,
    ROW_NUMBER() OVER (PARTITION BY p.id ORDER BY pte.execution_time DESC) AS rn
FROM pack p
LEFT JOIN pack_test_execution pte ON p.id = pte.pack_id
WHERE pte.id IS NOT NULL;

COMMENT ON VIEW pack_test_summary IS 'Summary of all pack test executions with pack details';

-- Latest test results per pack view
CREATE OR REPLACE VIEW pack_latest_test AS
SELECT
    pack_id,
    pack_ref,
    pack_label,
    test_execution_id,
    pack_version,
    test_time,
    trigger_reason,
    total_tests,
    passed,
    failed,
    skipped,
    pass_rate,
    duration_ms
FROM pack_test_summary
WHERE rn = 1;

COMMENT ON VIEW pack_latest_test IS 'Latest test results for each pack';

-- Function to get pack test statistics
CREATE OR REPLACE FUNCTION get_pack_test_stats(p_pack_id BIGINT)
RETURNS TABLE (
    total_executions BIGINT,
    successful_executions BIGINT,
    failed_executions BIGINT,
    avg_pass_rate DECIMAL,
    avg_duration_ms BIGINT,
    last_test_time TIMESTAMPTZ,
    last_test_passed BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        COUNT(*)::BIGINT AS total_executions,
        COUNT(*) FILTER (WHERE passed = total_tests)::BIGINT AS successful_executions,
        COUNT(*) FILTER (WHERE failed > 0)::BIGINT AS failed_executions,
        AVG(pass_rate) AS avg_pass_rate,
        AVG(duration_ms)::BIGINT AS avg_duration_ms,
        MAX(execution_time) AS last_test_time,
        (SELECT failed = 0 FROM pack_test_execution
         WHERE pack_id = p_pack_id
         ORDER BY execution_time DESC
         LIMIT 1) AS last_test_passed
    FROM pack_test_execution
    WHERE pack_id = p_pack_id;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION get_pack_test_stats IS 'Get statistical summary of test executions for a pack';

-- Function to check if pack has recent passing tests
CREATE OR REPLACE FUNCTION pack_has_passing_tests(
    p_pack_id BIGINT,
    p_hours_ago INT DEFAULT 24
)
RETURNS BOOLEAN AS $$
DECLARE
    v_has_passing_tests BOOLEAN;
BEGIN
    SELECT EXISTS(
        SELECT 1
        FROM pack_test_execution
        WHERE pack_id = p_pack_id
        AND execution_time > NOW() - (p_hours_ago || ' hours')::INTERVAL
        AND failed = 0
        AND total_tests > 0
    ) INTO v_has_passing_tests;

    RETURN v_has_passing_tests;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION pack_has_passing_tests IS 'Check if pack has recent passing test executions';

-- Add trigger to update pack metadata on test execution
CREATE OR REPLACE FUNCTION update_pack_test_metadata()
RETURNS TRIGGER AS $$
BEGIN
    -- Could update pack table with last_tested timestamp if we add that column
    -- For now, just a placeholder for future functionality
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_pack_test_metadata
    AFTER INSERT ON pack_test_execution
    FOR EACH ROW
    EXECUTE FUNCTION update_pack_test_metadata();

COMMENT ON TRIGGER trigger_update_pack_test_metadata ON pack_test_execution IS 'Updates pack metadata when tests are executed';
