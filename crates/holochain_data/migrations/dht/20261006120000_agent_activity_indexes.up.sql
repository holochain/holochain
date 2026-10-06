-- Indexes for the agent activity and record lookups (holochain#6007).
--
-- Without them every `must_get_agent_activity` call, every authority record
-- lookup and every integration-time `ChainOp` lookup by action hash scans the
-- whole `ChainOp` table.
CREATE INDEX Action_author_seq ON Action(author, seq);
CREATE INDEX ChainOp_action_hash_op_type ON ChainOp(action_hash, op_type);
