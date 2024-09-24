pub struct Ta<'txn, K: DbKindT> {
    txn: &'txn Transaction<'txn>,
    kind: PhantomData<K>,
}

impl<'txn, K: DbKindT> From<&'txn Transaction<'txn>> for Ta<'txn, K> {
    fn from(txn: &'txn Transaction<'txn>) -> Self {
        Ta {
            txn,
            kind: PhantomData,
        }
    }
}
