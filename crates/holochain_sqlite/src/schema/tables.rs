pub trait SqlInsert {
    fn sql_insert<R: Readable>(&self, txn: &mut R) -> DatabaseResult<()>;
}

impl SqlInsert for Entry {
    fn sql_insert<R: Readable>(&self, txn: &mut R) -> DatabaseResult<()> {}
}
