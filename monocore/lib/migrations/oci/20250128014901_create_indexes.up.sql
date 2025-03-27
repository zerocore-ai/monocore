-- Add up migration script here

-- Create indexes table
CREATE TABLE IF NOT EXISTS indexes (
    id INTEGER PRIMARY KEY,
    image_id INTEGER NOT NULL,
    schema_version INTEGER NOT NULL,
    media_type TEXT NOT NULL,
    platform_os TEXT,
    platform_arch TEXT,
    platform_variant TEXT,
    annotations_json TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (image_id) REFERENCES images(id) ON DELETE CASCADE
);

-- Create index
CREATE INDEX IF NOT EXISTS idx_indexes_image_id ON indexes(image_id);
