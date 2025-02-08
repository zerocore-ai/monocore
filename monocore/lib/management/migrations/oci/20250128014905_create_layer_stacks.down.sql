-- Add down migration script here
DROP INDEX IF EXISTS idx_layer_stack_entries_stack_id;
DROP INDEX IF EXISTS idx_layer_stack_entries_layer_id;
DROP TABLE IF EXISTS layer_stack_entries;
DROP TABLE IF EXISTS layer_stacks;
