-- Add down migration script here

DROP INDEX IF EXISTS idx_manifests_image_id;
DROP INDEX IF EXISTS idx_manifests_index_id;
DROP TABLE IF EXISTS manifests;
