-- Add down migration script here

DROP INDEX IF EXISTS idx_images_reference;
DROP TABLE IF EXISTS images;
