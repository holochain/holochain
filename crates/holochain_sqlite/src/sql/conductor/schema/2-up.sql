-- Per-role init properties supplied at install time.
-- Opaque, app-defined bytes read back during the cell's `init` callback.
-- Never written to the DHT.
CREATE TABLE IF NOT EXISTS InitProperties (
  app_id TEXT NOT NULL,
  role_name TEXT NOT NULL,
  properties BLOB NOT NULL,
  PRIMARY KEY (app_id, role_name)
) STRICT;
