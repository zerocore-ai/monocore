-- Add up migration script here

-- Create manifests table
CREATE TABLE IF NOT EXISTS manifests (
    id INTEGER PRIMARY KEY,
    index_id INTEGER,
    image_id INTEGER NOT NULL,
    schema_version INTEGER NOT NULL,
    media_type TEXT NOT NULL,
    annotations_json TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (index_id) REFERENCES indexes(id) ON DELETE CASCADE,
    FOREIGN KEY (image_id) REFERENCES images(id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_manifests_index_id ON manifests(index_id);
CREATE INDEX IF NOT EXISTS idx_manifests_image_id ON manifests(image_id);
