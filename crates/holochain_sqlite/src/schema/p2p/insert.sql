-- basic full insert
INSERT INTO p2p_store (
  space,
  agent,
  signed_at_ms,
  expires_at_ms,
  encoded,
  storage_center_loc,
  storage_half_length,
  storage_start_1,
  storage_end_1,
  storage_start_2,
  storage_end_2
) VALUES (
  :space,
  :agent,
  :signed_at_ms,
  :expires_at_ms,
  :encoded,
  :storage_center_loc,
  :storage_half_length,
  :storage_start_1,
  :storage_end_1,
  :storage_start_2,
  :storage_end_2
);
