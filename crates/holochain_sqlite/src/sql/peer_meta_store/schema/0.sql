CREATE TABLE peer_meta (
  peer_url TEXT NOT NULL,
  meta_key TEXT NOT NULL,
  meta_value BLOB NOT NULL,
  expires_at INTEGER,
  PRIMARY KEY (peer_url, meta_key) ON CONFLICT REPLACE
);
