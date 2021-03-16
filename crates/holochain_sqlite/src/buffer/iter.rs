use crate::buffer::kv::KvOp;
use crate::error::DatabaseError;
use crate::prelude::*;
use fallible_iterator::DoubleEndedFallibleIterator;
use fallible_iterator::FallibleIterator;
use rusqlite::*;
use std::collections::BTreeMap;
use tracing::*;

type IterItem<V> = (Vec<u8>, V);
type IterItemRef<'a, V> = (&'a [u8], V);
type IterError = DatabaseError;

fn item_owned<V>(item_ref: IterItemRef<V>) -> IterItem<V> {
    (item_ref.0.to_owned(), item_ref.1)
}

#[macro_export]
macro_rules! rewrap_iter {
    ($it:expr) => {
        $it.collect::<Vec<_>>().into_iter()
    };
}

#[macro_export]
macro_rules! rewrap_fallible_iter {
    ($it:expr) => {
        fallible_iterator::convert($it.collect::<Vec<_>>()?.into_iter().map(Ok))
    };
}

/// Returns all the elements on this key
pub struct SingleIterKeyMatch<'a, V>
where
    V: BufVal,
{
    iter: SingleIterFrom<'a, V>,
    key: Vec<u8>,
}

impl<'a, V> SingleIterKeyMatch<'a, V>
where
    V: BufVal,
{
    pub fn new(iter: SingleIterFrom<'a, V>, key: Vec<u8>) -> Self {
        Self { iter, key }
    }
}

impl<'a, V> FallibleIterator for SingleIterKeyMatch<'a, V>
where
    V: BufVal,
{
    type Error = DatabaseError;
    type Item = IterItem<V>;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let item = self.iter.next()?;
        match &item {
            Some((k, _)) if !partial_key_match(&self.key[..], k) => Ok(None),
            _ => Ok(item),
        }
    }
}

/// Match a key on another partial key
pub fn partial_key_match(partial_key: &[u8], key: &[u8]) -> bool {
    let len = partial_key.len();
    // Avoid slice panic
    key.get(0..len)
        .map(|a| a == &partial_key[..])
        .unwrap_or(false)
}

/// Iterate from a key
pub struct SingleIterFrom<'a, V>
where
    V: BufVal,
{
    iter: SingleIter<'a, V>,
}

impl<'a, V> SingleIterFrom<'a, V>
where
    V: BufVal,
{
    pub fn new(
        scratch: &'a BTreeMap<Vec<u8>, KvOp<V>>,
        iter: SingleIterRaw<V>,
        key: Vec<u8>,
    ) -> Self {
        let iter = SingleIter::new(&scratch, scratch.range(key..), iter);
        Self { iter }
    }
}

impl<'a, V> FallibleIterator for SingleIterFrom<'a, V>
where
    V: BufVal,
{
    type Error = DatabaseError;
    type Item = IterItem<V>;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        self.iter.next()
    }
}

/// Draining iterator that only touches the db on commit
pub struct DrainIter<'a, V>
where
    V: BufVal,
{
    scratch: &'a mut BTreeMap<Vec<u8>, KvOp<V>>,
    iter: Box<dyn DoubleEndedFallibleIterator<Item = IterItem<V>, Error = DatabaseError> + 'a>,
}

impl<'a, V> DrainIter<'a, V>
where
    V: BufVal,
{
    pub fn new(
        scratch: &'a mut BTreeMap<Vec<u8>, KvOp<V>>,
        iter: impl DoubleEndedFallibleIterator<Item = IterItem<V>, Error = DatabaseError> + 'a,
    ) -> Self {
        Self {
            scratch,
            iter: Box::new(iter),
        }
    }
}

impl<'a, V> FallibleIterator for DrainIter<'a, V>
where
    V: BufVal,
{
    type Error = DatabaseError;
    type Item = V;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.iter.next()?.map(|(k, v)| {
            self.scratch.insert(k.to_vec(), KvOp::Delete);
            v
        }))
    }
}

impl<'a, V> DoubleEndedFallibleIterator for DrainIter<'a, V>
where
    V: BufVal,
{
    fn next_back(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        Ok(self.iter.next_back()?.map(|(k, v)| {
            self.scratch.insert(k.to_vec(), KvOp::Delete);
            v
        }))
    }
}
/// Iterate taking into account the scratch
pub struct SingleIter<'a, V>
where
    V: BufVal,
{
    scratch_iter: Box<dyn DoubleEndedIterator<Item = IterItemRef<'a, V>> + 'a>,
    iter: Box<dyn DoubleEndedFallibleIterator<Item = IterItem<V>, Error = DatabaseError> + 'a>,
    current: Option<IterItem<V>>,
    scratch_current: Option<IterItemRef<'a, V>>,
}

impl<'a, V> SingleIter<'a, V>
where
    V: BufVal,
{
    pub fn new(
        scratch: &'a BTreeMap<Vec<u8>, KvOp<V>>,
        scratch_iter: impl DoubleEndedIterator<Item = (&'a Vec<u8>, &'a KvOp<V>)> + 'a,
        iter: SingleIterRaw<V>,
    ) -> Self {
        let scratch_iter = scratch_iter
            // TODO: These inspects should be eventally removed
            // but I'm tempted to included them for a while
            // incase any bugs are found in the iterator.
            // They make debugging a lot easier.
            .inspect(|(k, v)| {
                let span = trace_span!("scratch < filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v)
            })
            // Don't include deletes because they are handled
            // in the next db iterator
            .filter_map(|(k, v)| match v {
                KvOp::Put(v) => Some((&k[..], *v.clone())),
                KvOp::Delete => None,
            })
            .inspect(|(k, v)| {
                let span = trace_span!("scratch > filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v)
            });

        // Raw iter
        let iter = iter
            .inspect(|(k, v)| {
                let span = trace_span!("db < filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v);
                Ok(())
            })
            // Remove items that match a delete in the scratch.
            // If there is a put in the scratch we want to return
            // that instead of this matching item as the scratch
            // is more up to date
            .filter_map(move |(k, v)| match scratch.get(&k) {
                Some(KvOp::Put(sv)) => Ok(Some((k, *sv.clone()))),
                Some(KvOp::Delete) => Ok(None),
                None => Ok(Some((k, v))),
            })
            .inspect(|(k, v)| {
                let span = trace_span!("db > filter", key = %String::from_utf8_lossy(k));
                let _g = span.enter();
                trace!(k = %String::from_utf8_lossy(k), ?v);
                Ok(())
            });
        Self {
            scratch_iter: Box::new(scratch_iter),
            iter: Box::new(iter),
            current: None,
            scratch_current: None,
        }
    }

    fn check_scratch(
        &mut self,
        scratch_current: Option<IterItemRef<'a, V>>,
        db: IterItem<V>,
        compare: fn(scratch: &[u8], db: &[u8]) -> bool,
    ) -> Option<IterItem<V>> {
        match scratch_current {
            // Return scratch value and keep db value
            Some(scratch) if compare(scratch.0, db.0.as_slice()) => {
                trace!(msg = "r scratch key first", k = %String::from_utf8_lossy(&scratch.0[..]), v = ?scratch.1);
                self.current = Some(db);
                Some(item_owned(scratch))
            }
            // Return scratch value (or db value) and throw the other away
            Some(scratch) if scratch.0 == db.0 => {
                trace!(msg = "r scratch key ==", k = %String::from_utf8_lossy(&scratch.0[..]), v = ?scratch.1);
                Some(item_owned(scratch))
            }
            // Return db value and keep the scratch
            _ => {
                trace!(msg = "r db _", k = %String::from_utf8_lossy(&db.0[..]), v = ?db.1);
                self.scratch_current = scratch_current;
                Some(db)
            }
        }
    }

    fn next_inner(
        &mut self,
        current: Option<IterItem<V>>,
        scratch_current: Option<IterItemRef<'a, V>>,
        compare: fn(scratch: &[u8], db: &[u8]) -> bool,
    ) -> Result<Option<IterItem<V>>, IterError> {
        let r = match current {
            Some(db) => self.check_scratch(scratch_current, db, compare),
            None => {
                if let Some((k, v)) = &scratch_current {
                    trace!(msg = "r scratch no db", k = %String::from_utf8_lossy(k), ?v);
                } else {
                    trace!("r None")
                }
                scratch_current.map(item_owned)
            }
        };
        Ok(r)
    }
}

impl<'a, V> FallibleIterator for SingleIter<'a, V>
where
    V: BufVal,
{
    type Error = IterError;
    type Item = IterItem<V>;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let current = match self.current.take() {
            Some(c) => Some(c),
            None => self.iter.next()?,
        };
        let scratch_current = match self.scratch_current.take() {
            Some(c) => Some(c),
            None => self.scratch_iter.next(),
        };
        self.next_inner(current, scratch_current, |scratch, db| scratch < db)
    }
}

impl<'a, V> DoubleEndedFallibleIterator for SingleIter<'a, V>
where
    V: BufVal,
{
    fn next_back(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let current = match self.current.take() {
            Some(c) => Some(c),
            None => self.iter.next_back()?,
        };
        let scratch_current = match self.scratch_current.take() {
            Some(c) => Some(c),
            None => self.scratch_iter.next_back(),
        };
        self.next_inner(current, scratch_current, |scratch, db| scratch > db)
    }
}

pub type SqlIter =
    Box<dyn Iterator<Item = rusqlite::Result<(Vec<u8>, Option<rusqlite::types::Value>)>>>;

pub struct SingleIterRaw<V> {
    iter_front: SqlIter,
    iter_back: SqlIter,
    key: Option<Vec<u8>>,
    key_back: Option<Vec<u8>>,
    __type: std::marker::PhantomData<V>,
}

type InnerItem = (Vec<u8>, Option<rusqlite::types::Value>);

impl<V> SingleIterRaw<V>
where
    V: BufVal,
{
    pub fn new(iter_front: SqlIter, iter_back: SqlIter) -> Self {
        Self {
            iter_front,
            iter_back,
            key: None,
            key_back: None,
            __type: std::marker::PhantomData,
        }
    }

    fn next_inner(
        item: Option<Result<InnerItem, DatabaseError>>,
    ) -> Result<Option<IterItem<V>>, IterError> {
        match item {
            Some(Ok((k, Some(rusqlite::types::Value::Blob(buf))))) => Ok(Some((
                k,
                holochain_serialized_bytes::decode(&buf).expect(
                    "Failed to deserialize data from database. Database might be corrupted",
                ),
            ))),
            None => Ok(None),
            // TODO: Should this panic aswell?
            Some(Ok(_)) => Err(DatabaseError::InvalidValue),
            // This could be a IO error so returning it makes sense
            Some(Err(e)) => Err(e),
        }
    }
}

/// Iterate over key, value pairs in this store using low-level iterators
/// NOTE: While the value is deserialized to the proper type, the key is returned as raw bytes.
/// This is to enable a wider range of keys, such as String, because there is no uniform trait which
/// enables conversion from a byte slice to a given type.
impl<V> FallibleIterator for SingleIterRaw<V>
where
    V: BufVal,
{
    type Error = IterError;
    type Item = IterItem<V>;

    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let n = self
            .iter_front
            .next()
            .map(|o| o.map_err(DatabaseError::from));
        let r = Self::next_inner(n);
        if let Ok(Some((k, _))) = &r {
            // FIXME: probably unnecessary clone
            self.key = Some(k.clone());
            match &self.key_back {
                Some(k_back) if k >= k_back => return Ok(None),
                _ => {}
            }
        }
        r
    }
}

impl<V> DoubleEndedFallibleIterator for SingleIterRaw<V>
where
    V: BufVal,
{
    fn next_back(&mut self) -> Result<Option<Self::Item>, Self::Error> {
        let n = self
            .iter_back
            .next()
            .map(|o| o.map_err(DatabaseError::from));
        let r = Self::next_inner(n);
        if let Ok(Some((k_back, _))) = &r {
            // FIXME: probably unnecessary clone
            self.key_back = Some(k_back.clone());
            match &self.key {
                Some(key) if k_back <= key => return Ok(None),
                _ => {}
            }
        }
        r
    }
}
