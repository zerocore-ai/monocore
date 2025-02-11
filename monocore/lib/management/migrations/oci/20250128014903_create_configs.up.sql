-- Add up migration script here

-- Create configs table
CREATE TABLE IF NOT EXISTS configs (
    id INTEGER PRIMARY KEY,
    manifest_id INTEGER NOT NULL,
    media_type TEXT NOT NULL,

    -- Root level fields
    created DATETIME,
    architecture TEXT,
    os TEXT,
    os_variant TEXT,

    -- Config section fields
    config_env_json TEXT,
    config_cmd_json TEXT,
    config_working_dir TEXT,
    config_entrypoint_json TEXT,
    config_volumes_json TEXT,
    config_exposed_ports_json TEXT,
    config_user TEXT,

    -- Rootfs section
    rootfs_type TEXT,
    rootfs_diff_ids_json TEXT,

    history_json TEXT,

    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (manifest_id) REFERENCES manifests(id) ON DELETE CASCADE
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_configs_manifest_id ON configs(manifest_id);
