-- no-sql-format --

ALTER TABLE DhtOp ADD COLUMN  when_sys_validated  INTEGER  NULL;
ALTER TABLE DhtOp ADD COLUMN  when_app_validated  INTEGER  NULL;
ALTER TABLE DhtOp ADD COLUMN  when_stored         INTEGER  NULL;  -- Really should be NOT NULL but need to migrate data.
ALTER TABLE DhtOp ADD COLUMN  transfer_source     BLOB     NULL;  -- AgentPubKey we fetched from. Really should be NOT NULL but need to migrate data.
ALTER TABLE DhtOp ADD COLUMN  transfer_method     INTEGER  NULL;  -- TransferMethod by which the op hash was originally conveyed to us. Really should be NOT NULL but need to migrate data.
ALTER TABLE DhtOp ADD COLUMN  transfer_time       INTEGER  NULL;  -- Time that the op hash was originally conveyed to us. Really should be NOT NULL but need to migrate data.

UPDATE DhtOp SET when_stored = authored_timestamp;


ALTER TABLE ValidationReceipt ADD COLUMN  when_received        INTEGER  NULL;