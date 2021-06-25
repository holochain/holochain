-- p2p store
CREATE TABLE IF NOT EXISTS p2p_metrics (
  agent BLOB NOT NULL,
  kind TEXT NOT NULL,
  moment INTEGER NOT NULL,
  PRIMARY KEY (agent, kind, moment)
);
