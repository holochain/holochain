-- Drop column "dependency" and related indexes from table DhtOp.
DROP INDEX DhtOp_type_dep_idx;

DROP INDEX DhtOp_validation_stage_idx;

ALTER TABLE
  DhtOp DROP COLUMN dependency;