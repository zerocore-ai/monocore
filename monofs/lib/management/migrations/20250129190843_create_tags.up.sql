-- Add up migration script here

-- Create tags table
CREATE TABLE IF NOT EXISTS tags (
    id INTEGER PRIMARY KEY,
    fs_id INTEGER NOT NULL,
    root_revision TEXT NOT NULL,
    path TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (fs_id) REFERENCES filesystems(id) ON DELETE CASCADE
);

-- Create indexes for common queries
CREATE INDEX idx_tags_fs_id ON tags(fs_id);
CREATE INDEX idx_tags_name ON tags(name);
CREATE INDEX idx_tags_path ON tags(path);
