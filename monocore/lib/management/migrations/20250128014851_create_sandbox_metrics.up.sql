-- Add up migration script here

-- Create sandbox_metrics table
CREATE TABLE IF NOT EXISTS sandbox_metrics (
    id INTEGER PRIMARY KEY,
    sandbox_id INTEGER NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    cpu_usage REAL,
    memory_usage INTEGER,
    disk_read_bytes INTEGER,
    disk_write_bytes INTEGER,
    total_disk_read INTEGER,
    total_disk_write INTEGER,
    FOREIGN KEY(sandbox_id) REFERENCES sandbox(id)
);

-- Create index
CREATE INDEX IF NOT EXISTS idx_sandbox_metrics_sandbox_id_timestamp ON sandbox_metrics(sandbox_id, timestamp);
