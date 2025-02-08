-- Add up migration script here

-- Create images table
CREATE TABLE IF NOT EXISTS images (
    id INTEGER PRIMARY KEY,
    reference TEXT NOT NULL UNIQUE,
    size_bytes INTEGER NOT NULL,
    head_cid TEXT,
    last_used_at DATETIME,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create index
CREATE INDEX IF NOT EXISTS idx_images_reference ON images(reference);
