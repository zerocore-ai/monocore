-- Add up migration script here

-- Create filesystems table
CREATE TABLE IF NOT EXISTS filesystems (
    id INTEGER PRIMARY KEY,
    sandbox_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    supervisor_pid INTEGER,
    overlayfs_pid INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(sandbox_id) REFERENCES sandboxes(id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_filesystems_name ON filesystems(name);
CREATE INDEX IF NOT EXISTS idx_filesystems_sandbox_id ON filesystems(sandbox_id);
