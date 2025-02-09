-- Add up migration script here

-- Create layer chains table
CREATE TABLE IF NOT EXISTS layer_chains (
    id INTEGER PRIMARY KEY,
    chain_id TEXT NOT NULL UNIQUE, -- the hash of the ordered layers diff ids
    head_cid TEXT NOT NULL, -- the root cid of the combined fs in monofs
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Create layer chain entries table for the ordered layers
CREATE TABLE IF NOT EXISTS layer_chain_entries (
    id INTEGER PRIMARY KEY,
    chain_id INTEGER NOT NULL,
    layer_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    modified_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (chain_id) REFERENCES layer_chains(id) ON DELETE CASCADE,
    FOREIGN KEY (layer_id) REFERENCES layers(id) ON DELETE CASCADE,
    UNIQUE(chain_id, position),
    UNIQUE(chain_id, layer_id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_layer_chain_entries_chain_id ON layer_chain_entries(chain_id);
CREATE INDEX IF NOT EXISTS idx_layer_chain_entries_layer_id ON layer_chain_entries(layer_id);
CREATE INDEX IF NOT EXISTS idx_layer_chains_chain_id ON layer_chains(chain_id);
