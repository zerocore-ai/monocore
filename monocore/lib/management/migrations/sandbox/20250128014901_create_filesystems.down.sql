-- Add down migration script here

-- Drop indexes first
DROP INDEX IF EXISTS idx_filesystems_name;
DROP INDEX IF EXISTS idx_filesystems_sandbox_id;

-- Drop filesystems table
DROP TABLE IF EXISTS filesystems;
