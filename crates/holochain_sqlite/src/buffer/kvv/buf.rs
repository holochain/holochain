use crate::buffer::BufferedStore;
use crate::error::DatabaseError;
use crate::error::DatabaseResult;
use crate::prelude::*;
use either::Either;
use std::collections::BTreeMap;
use std::fmt::Debug;
use tracing::*;

#[cfg(test)]
mod tests;

/// Transactional operations on a KVV store
///
/// Replace is a Delete followed by an Insert
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum KvvOp {
    Insert,
    Delete,
}

#[derive(Clone)]
pub(super) struct ValuesDelta<V> {
    delete_all: bool,
    deltas: BTreeMap<V, KvvOp>,
}

impl<V: Ord + Eq> ValuesDelta<V> {
    fn all_deleted() -> Self {
        Self {
            delete_all: true,
            deltas: BTreeMap::new(),
        }
    }
}

// This would be equivalent to the derived impl, except that this
// doesn't require `V: Default`
impl<V: Ord + Eq> Default for ValuesDelta<V> {
    fn default() -> Self {
        Self {
            delete_all: bool::default(),
            deltas: BTreeMap::new(),
        }
    }
}

/// A persisted key-value store with a transient BTreeMap to store
/// CRUD-like changes without opening a blocking read-write cursor
pub struct KvvBufUsed<K, V>
where
    K: BufKey,
    V: BufMultiVal,
{
    db: MultiTable,
    scratch: BTreeMap<K, ValuesDelta<V>>,
    no_dup_data: bool,
}

impl<K, V> KvvBufUsed<K, V>
where
    K: BufKey + Debug,
    V: BufMultiVal + Debug,
{
    /// Create a new KvvBufUsed
    pub fn new(db: MultiTable) -> Self {
        Self::new_opts(db, false)
    }

    /// Create a new KvvBufUsed
    /// also allow switching to no_dup_data mode.
    pub fn new_opts(db: MultiTable, no_dup_data: bool) -> Self {
        Self {
            db,
            scratch: BTreeMap::new(),
            no_dup_data,
        }
    }

    // /// Get a set of values, taking the scratch space into account,
    // /// or from persistence if needed
    // #[instrument(skip(self, r))]
    // pub fn get<'r, R: Readable, KK: 'r + Debug + AsRef<K>>(
    //     &'r self,
    //     r: &'r R,
    //     k: KK,
    // ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + 'r> {

    /// Get a set of values, taking the scratch space into account,
    /// or from persistence if needed
    #[instrument(skip(self, r))]
    pub fn get<'r, R: Readable, KK: 'r + Debug + std::borrow::Borrow<K>>(
        &'r self,
        r: &'r R,
        k: KK,
    ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + 'r> {
        todo!("Revisit later, too much to consider for the current basic type refactor");
        // Depending on which branches get taken, this function could return
        // any of three different iterator types, in order to unify all three
        // into a single type, we return (in the happy path) a value of type
        // ```
        // Either<__GetPersistedIter, Either<__ScratchSpaceITer, Chain<...>>>
        // ```

        let values_delta = if let Some(v) = self.scratch.get(k.borrow()) {
            v
        } else {
            // Only do the persisted call if it's not in the scratch
            trace!(?k);
            let persisted = Self::check_not_found(self.get_persisted(r, k.borrow()))?;

            return Ok(Either::Left(persisted));
        };
        let ValuesDelta { delete_all, deltas } = values_delta;

        let from_scratch_space = deltas
            .iter()
            .filter(|(_v, op)| **op == KvvOp::Insert)
            .map(|(v, _op)| Ok(v.clone()));

        let iter = if *delete_all {
            // If delete_all is set, return only scratch content,
            // skipping persisted content (as it will all be deleted)
            Either::Left(from_scratch_space)
        } else {
            let persisted = Self::check_not_found(self.get_persisted(r, k.borrow()))?;
            Either::Right(
                from_scratch_space
                    // Otherwise, chain it with the persisted content,
                    // skipping only things that we've specifically deleted or returned.
                    .chain(persisted.filter(move |r| match r {
                        Ok(v) => !deltas.contains_key(v),
                        Err(_e) => true,
                    })),
            )
        };

        Ok(Either::Right(iter))
    }

    /// Update the scratch space to record an Insert operation for the KV
    pub fn insert(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .or_default()
            .deltas
            .insert(v, KvvOp::Insert);
    }

    /// Update the scratch space to record a Delete operation for the KV
    pub fn delete(&mut self, k: K, v: V) {
        self.scratch
            .entry(k)
            .or_default()
            .deltas
            .insert(v, KvvOp::Delete);
    }

    /// Clear the scratch space and record a DeleteAll operation
    pub fn delete_all(&mut self, k: K) {
        self.scratch.insert(k, ValuesDelta::all_deleted());
    }

    /// Fetch data from DB, deserialize into V type
    #[instrument(skip(self, r))]
    fn get_persisted<'r, R: Readable>(
        &'r self,
        r: &'r R,
        k: &'r K,
    ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>> + 'r> {
        let s = trace_span!("persisted");
        let _g = s.enter();
        trace!("test");
        let iter = self.db.get_m(r, k)?;
        Ok(iter.filter_map(|v| match v {
            Ok((_, Some(rkv::Value::Blob(buf)))) => Some(
                holochain_serialized_bytes::decode(buf)
                    .map(|n| {
                        trace!(?n);
                        n
                    })
                    .map_err(|e| e.into()),
            ),
            Ok((_, Some(_))) => Some(Err(DatabaseError::InvalidValue)),
            Ok((_, None)) => None,
            Err(e) => Some(Err(e.into())),
        }))
    }

    fn check_not_found(
        persisted: DatabaseResult<impl Iterator<Item = DatabaseResult<V>>>,
    ) -> DatabaseResult<impl Iterator<Item = DatabaseResult<V>>> {
        let empty = std::iter::empty::<DatabaseResult<V>>();
        trace!("{:?}", line!());

        match persisted {
            Ok(persisted) => {
                trace!("{:?}", line!());
                Ok(Either::Left(persisted))
            }
            Err(err) => {
                trace!("{:?}", line!());
                err.ok_if_not_found()?;
                Ok(Either::Right(empty))
            }
        }
    }

    // TODO: This should be cfg test but can't because it's in a different crate
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.scratch.clear();
        Ok(self.db.clear(writer)?)
    }
}

impl<K, V> BufferedStore for KvvBufUsed<K, V>
where
    K: Clone + BufKey + Debug,
    V: BufMultiVal + Debug,
{
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.scratch.is_empty()
    }

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        use KvvOp::*;
        if self.is_clean() {
            return Ok(());
        }
        for (k, ValuesDelta { delete_all, deltas }) in self.scratch.iter() {
            // If delete_all is set, that we should delete everything persisted,
            // but then continue to add inserts from the ops, if present
            if *delete_all {
                self.db.delete_all(writer, k.clone())?;
            }
            trace!(?k);
            trace!(?deltas);

            for (v, op) in deltas {
                match op {
                    Insert => {
                        let buf = holochain_serialized_bytes::encode(&v)?;
                        let encoded = rkv::Value::Blob(&buf);
                        if self.no_dup_data {
                            self.db
                                .put_with_flags(
                                    writer,
                                    k.clone(),
                                    &encoded,
                                    rkv::WriteFlags::NO_DUP_DATA,
                                )
                                .or_else(|err| {
                                    todo!(
                                        "remove this, should be unnecessary in the context of SQL"
                                    );
                                    StoreResult::Ok(())
                                    // // This error is a little misleading...
                                    // // In a MultiTable with NO_DUP_DATA, it is
                                    // // actually returned if there is a duplicate
                                    // // value... which we want to ignore.
                                    // if let rkv::StoreError::LmdbError(rkv::LmdbError::KeyExist) =
                                    //     err
                                    // {
                                    //     Ok(())
                                    // } else {
                                    //     Err(err)
                                    // }
                                })?;
                        } else {
                            self.db.put(writer, k.clone(), &encoded)?;
                        }
                    }
                    // Skip deleting unnecessarily if we have already deleted
                    // everything
                    Delete if *delete_all => {}
                    Delete => {
                        let buf = holochain_serialized_bytes::encode(&v)?;
                        let encoded = rkv::Value::Blob(&buf);
                        self.db
                            .delete_m(writer, k.clone(), &encoded)
                            .or_else(StoreError::ok_if_not_found)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Create an KvvBufUsed with a clone of the scratch
/// from another KvvBufUsed
impl<K, V> From<&KvvBufUsed<K, V>> for KvvBufUsed<K, V>
where
    K: BufKey + Debug + Clone,
    V: BufMultiVal + Debug,
{
    fn from(other: &KvvBufUsed<K, V>) -> Self {
        Self {
            db: other.db.clone(),
            scratch: other.scratch.clone(),
            no_dup_data: other.no_dup_data,
        }
    }
}
