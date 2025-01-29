-- Add down migration script here

-- Drop index first
DROP INDEX IF EXISTS idx_groups_name;

-- Drop groups table
DROP TABLE IF EXISTS groups;
