-- Add up migration script here

-- Create layers table
CREATE TABLE IF NOT EXISTS layers (
    id INTEGER PRIMARY KEY,
    manifest_id INTEGER NOT NULL,
    media_type TEXT NOT NULL,
    digest TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    diff_id TEXT,
    head_cid TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (manifest_id) REFERENCES manifests(id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_layers_manifest_id ON layers(manifest_id);
CREATE INDEX IF NOT EXISTS idx_layers_digest ON layers(digest);
