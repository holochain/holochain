-- Add up migration script here
-- Initial sample schema
CREATE TABLE sample_data (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    value TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX idx_sample_data_name ON sample_data(name);
