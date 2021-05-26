-- p2p store
CREATE TABLE IF NOT EXISTS p2p_metrics (
  agent BLOB NOT NULL,
  metric TEXT NOT NULL,
  timestamp INTEGER NOT NULL,
  CONSTRAINT PRIMARY KEY (agent, metric, timestamp)
);
