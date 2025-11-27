-- Add down migration script here
-- Revert initial sample schema
DROP INDEX IF EXISTS idx_sample_data_name;
DROP TABLE IF EXISTS sample_data;
