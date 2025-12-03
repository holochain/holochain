-- Add down migration script here
-- Revert initial sample schema
DROP INDEX IF EXISTS idx_sample_data_name;
DROP TABLE IF EXISTS sample_data;
-- Revert Wasm database schema
DROP TABLE IF EXISTS CoordinatorZome;
DROP TABLE IF EXISTS IntegrityZome;
DROP TABLE IF EXISTS EntryDef;
DROP TABLE IF EXISTS DnaDef;
DROP TABLE IF EXISTS Wasm;
-- Revert Conductor database schema
DROP TABLE IF EXISTS SignalSubscription;
DROP TABLE IF EXISTS AppInterface;
DROP TABLE IF EXISTS CloneCell;
DROP TABLE IF EXISTS AppRole;
DROP TABLE IF EXISTS InstalledApp;
DROP TABLE IF EXISTS Conductor;