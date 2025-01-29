-- Add down migration script here

-- Drop indexes
DROP INDEX IF EXISTS idx_tags_fs_id;
DROP INDEX IF EXISTS idx_tags_name;
DROP INDEX IF EXISTS idx_tags_path;

-- Drop table
DROP TABLE IF EXISTS tags;
