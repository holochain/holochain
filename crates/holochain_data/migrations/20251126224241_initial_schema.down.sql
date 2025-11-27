-- Add down migration script here
-- Revert initial sample schema
-- Revert Wasm database schema
DROP INDEX IF EXISTS idx_sample_data_name;
DROP TABLE IF EXISTS sample_data;
DROP TABLE IF EXISTS CoordinatorZome;
DROP TABLE IF EXISTS IntegrityZome;
DROP TABLE IF EXISTS EntryDef;
DROP TABLE IF EXISTS DnaDef;
DROP TABLE IF EXISTS Wasm;
