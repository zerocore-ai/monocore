-- Add up migration script here

-- Create filesystems table
CREATE TABLE IF NOT EXISTS filesystems (
    id INTEGER PRIMARY KEY,
    head TEXT NOT NULL,
    mount_dir TEXT NOT NULL,
    supervisor_pid INTEGER,
    nfsserver_pid INTEGER,
    config TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Create index for mount_dir lookups
CREATE INDEX idx_filesystems_mount_dir ON filesystems(mount_dir);
