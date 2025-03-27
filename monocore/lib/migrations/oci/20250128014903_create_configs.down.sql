-- Add down migration script here

DROP INDEX IF EXISTS idx_configs_fingerprint;
DROP INDEX IF EXISTS idx_configs_manifest_id;
DROP TABLE IF EXISTS configs;
