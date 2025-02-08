-- Add up migration script here

-- Create layer stacks table
CREATE TABLE IF NOT EXISTS layer_stacks (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create layer stack entries table for the ordered layers
CREATE TABLE IF NOT EXISTS layer_stack_entries (
    id INTEGER PRIMARY KEY,
    stack_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (stack_id) REFERENCES layer_stacks(id) ON DELETE CASCADE,
    FOREIGN KEY (layer_id) REFERENCES layers(id) ON DELETE CASCADE,
    UNIQUE(stack_id, position),
    UNIQUE(stack_id, layer_id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_layer_stack_entries_stack_id ON layer_stack_entries(stack_id);
CREATE INDEX IF NOT EXISTS idx_layer_stack_entries_layer_id ON layer_stack_entries(layer_id);
