-- Drop conductor schema tables in reverse dependency order
DROP TABLE IF EXISTS SignalSubscription;
DROP TABLE IF EXISTS AppInterface;
DROP TABLE IF EXISTS CloneCell;
DROP TABLE IF EXISTS AppRole;
DROP TABLE IF EXISTS InstalledApp;
DROP TABLE IF EXISTS Conductor;
