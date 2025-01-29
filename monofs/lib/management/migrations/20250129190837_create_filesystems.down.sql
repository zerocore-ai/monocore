-- Add down migration script here

-- Drop index
DROP INDEX IF EXISTS idx_filesystems_mount_dir;

-- Drop table
DROP TABLE IF EXISTS filesystems;
