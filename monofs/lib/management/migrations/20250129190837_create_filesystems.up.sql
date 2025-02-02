-- Add up migration script here

-- Create filesystems table
CREATE TABLE IF NOT EXISTS filesystems (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    supervisor_pid INTEGER,
    nfsserver_pid INTEGER,
    head TEXT,
    mount_dir TEXT NOT NULL,
    config TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index for mount_dir lookups
CREATE INDEX idx_filesystems_mount_dir ON filesystems(mount_dir);
