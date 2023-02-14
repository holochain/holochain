CREATE TABLE IF NOT EXISTS BlockSpan (
    id INTEGER PRIMARY KEY,

    target_id BLOB NOT NULL,
    target_reason BLOB NOT NULL,

    -- start and end micros
    -- literal integer from Timestamp in rust
    start_us INTEGER NOT NULL,
    end_us INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS block_span_start_us_idx ON BlockSpan(start_us);
CREATE INDEX IF NOT EXISTS block_span_end_us_idx ON BlockSpan(end_us);
