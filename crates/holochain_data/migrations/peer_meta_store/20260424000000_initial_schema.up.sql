-- Peer metadata store schema
-- Stores arbitrary key-value metadata about peers, with optional expiry.

CREATE TABLE IF NOT EXISTS peer_meta (
    peer_url TEXT NOT NULL,
    meta_key TEXT NOT NULL,
    meta_value BLOB NOT NULL,
    expires_at INTEGER,
    PRIMARY KEY (peer_url, meta_key) ON CONFLICT REPLACE
) STRICT;

CREATE INDEX IF NOT EXISTS meta_key_idx ON peer_meta (meta_key);
CREATE INDEX IF NOT EXISTS expires_at_idx ON peer_meta (expires_at) WHERE expires_at IS NOT NULL;
