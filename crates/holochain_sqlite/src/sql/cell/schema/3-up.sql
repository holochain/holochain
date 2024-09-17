-- no-sql-format --

ALTER TABLE DhtOp ADD COLUMN  when_sys_validated  INTEGER  NULL;
ALTER TABLE DhtOp ADD COLUMN  when_app_validated  INTEGER  NULL;
ALTER TABLE DhtOp ADD COLUMN  when_stored         INTEGER  NULL;  -- Really should be NOT NULL but need to migrate data
UPDATE DhtOp SET when_stored = authored_timestamp;


ALTER TABLE ValidationReceipt ADD COLUMN  when_received        INTEGER  NULL;
