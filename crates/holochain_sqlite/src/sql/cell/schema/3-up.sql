-- no-sql-format --

ALTER TABLE DhtOp ADD COLUMN  when_sys_validated  INTEGER  NULL;
ALTER TABLE DhtOp ADD COLUMN  when_app_validated  INTEGER  NULL;
