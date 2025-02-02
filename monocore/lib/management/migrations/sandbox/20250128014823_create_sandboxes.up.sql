-- Add up migration script here

-- Create sandboxes table
CREATE TABLE IF NOT EXISTS sandboxes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    supervisor_pid INTEGER,
    microvm_pid INTEGER,
    status TEXT NOT NULL,
    rootfs_path TEXT NOT NULL,
    group_id INTEGER,
    group_ip TEXT,
    config TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(group_id) REFERENCES groups(id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_sandboxes_name ON sandboxes(name);
CREATE INDEX IF NOT EXISTS idx_sandboxes_group_id ON sandboxes(group_id);
