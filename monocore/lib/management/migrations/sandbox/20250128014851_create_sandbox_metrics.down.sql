-- Add down migration script here

-- Drop index first
DROP INDEX IF EXISTS idx_sandbox_metrics_sandbox_id_timestamp;

-- Drop sandbox_metrics table
DROP TABLE IF EXISTS sandbox_metrics;
