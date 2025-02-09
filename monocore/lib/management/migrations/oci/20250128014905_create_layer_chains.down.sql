-- Add down migration script here
DROP INDEX IF EXISTS idx_layer_chain_entries_chain_id;
DROP INDEX IF EXISTS idx_layer_chain_entries_layer_id;
DROP INDEX IF EXISTS idx_layer_chains_chain_id;
DROP TABLE IF EXISTS layer_chain_entries;
DROP TABLE IF EXISTS layer_chains;
