use lmdb;

pub type RoCursor<'txn> = lmdb::RoCursor<'txn>;
pub type RwCursor<'txn> = lmdb::RwCursor<'txn>;
pub type RoTransaction<'txn> = lmdb::RoTransaction<'txn>;
pub type RwTransaction<'txn> = lmdb::RwTransaction<'txn>;
pub type Database = lmdb::Database;
