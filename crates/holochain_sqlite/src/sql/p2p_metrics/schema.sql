-- no-sql-format --

CREATE TABLE IF NOT EXISTS p2p_metrics (
    -- just explicitly list the rowid, it'll be there anyways...
    rowid                  INTEGER PRIMARY KEY UNIQUE NOT NULL,

    -- text identifier for the type of metric recorded
    kind                   TEXT NOT NULL,

    -- the remote agent this metric is related to, if any
    agent                  BLOB NULL,

    -- the time at which this metric was logged
    recorded_at_utc_micros INTEGER NOT NULL,

    -- the time after which this metric can be pruned
    expires_at_utc_micros  INTEGER NOT NULL,

    -- any additional json encoded data associated
    -- with this metric
    data                   TEXT NULL
);

CREATE INDEX IF NOT EXISTS p2p_metrics_kind_idx
  ON p2p_metrics (kind);

CREATE INDEX IF NOT EXISTS p2p_metrics_agent_idx
  ON p2p_metrics (agent);

CREATE INDEX IF NOT EXISTS p2p_metrics_rec_at_idx
  ON p2p_metrics (recorded_at_utc_micros);

CREATE INDEX IF NOT EXISTS p2p_metrics_exp_at_idx
  ON p2p_metrics (expires_at_utc_micros);
