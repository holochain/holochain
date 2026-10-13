-- Indexes for the hot DHT read paths (holochain#5726).
--
-- `Action(author, seq)` and `ChainOp(action_hash, op_type)` already exist
-- from the agent activity migration; the latter also serves lookups by
-- `ChainOp.action_hash` alone.
CREATE INDEX Action_prev_hash ON Action(prev_hash);
CREATE INDEX ChainOp_basis_hash_op_type ON ChainOp(basis_hash, op_type);
CREATE INDEX ChainOp_when_integrated_locally_validated
    ON ChainOp(when_integrated) WHERE locally_validated = 1;
CREATE INDEX Link_base_hash ON Link(base_hash);
CREATE INDEX DeletedLink_create_link_hash ON DeletedLink(create_link_hash);
CREATE INDEX UpdatedRecord_original_action_hash ON UpdatedRecord(original_action_hash);
CREATE INDEX UpdatedRecord_original_entry_hash ON UpdatedRecord(original_entry_hash);
CREATE INDEX DeletedRecord_deletes_action_hash ON DeletedRecord(deletes_action_hash);
CREATE INDEX DeletedRecord_deletes_entry_hash ON DeletedRecord(deletes_entry_hash);
CREATE INDEX ValidationReceipt_op_hash ON ValidationReceipt(op_hash);
