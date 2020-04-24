#![allow(clippy::ptr_arg)]
use holochain_state::{
    buffer::{BufMultiVal, KvvBuf},
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::DatabaseResult,
    prelude::*,
};
use holochain_types::entry::EntryAddress;
use mockall::mock;
use std::collections::HashSet;
use std::fmt::Debug;

type Tag = String;

#[derive(Debug)]
pub enum EntryDhtStatus {
    Live,
    Dead,
    Pending,
    Rejected,
    Abandoned,
    Conflict,
    Withdrawn,
    Purged,
}

#[allow(dead_code)]
enum Op {
    Add,
    Remove,
}

#[allow(dead_code)]
struct LinkKey<'a> {
    base: &'a EntryAddress,
    op: Op,
    tag: Tag,
}

impl<'a> LinkKey<'a> {
    fn to_key(&self) -> Vec<u8> {
        let mut vec: Vec<u8> = self.base.as_ref().to_vec();
        vec.extend_from_slice(self.tag.as_ref());
        vec
    }
}

/*
TODO impliment these types
AddLink:
Base: hash
Target: hash
Type: (maybe?)
Tag: string
Addlink_Time: timestamp
Addlink_Action: hash

RemoveLink:
Base:
Target:
Type:
Tag:
AddLink_Time: timestamp
AddLink_Action: hash
RemoveLink_Action: timestamp
RemoveLink_Action: hash
*/

pub trait ChainMetaBufT<'env, R = Reader<'env>>
where
    R: Readable,
{
    fn get_links(&self, base: EntryAddress, tag: Tag) -> DatabaseResult<HashSet<EntryAddress>>;
    fn get_crud(&self, entry_address: EntryAddress) -> DatabaseResult<EntryDhtStatus>;
}
pub struct ChainMetaBuf<'env, V, R = Reader<'env>>
where
    V: BufMultiVal,
    R: Readable,
{
    _system_meta: KvvBuf<'env, Vec<u8>, V, R>,
    links_meta: KvvBuf<'env, Vec<u8>, V, R>,
}

impl<'env, V, R> ChainMetaBuf<'env, V, R>
where
    V: BufMultiVal + Debug,
    R: Readable,
{
    pub(crate) fn new(
        reader: &'env R,
        system_meta: MultiStore,
        links_meta: MultiStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            _system_meta: KvvBuf::new(reader, system_meta)?,
            links_meta: KvvBuf::new(reader, links_meta)?,
        })
    }
    pub fn primary(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        let system_meta = *dbs.get(&*PRIMARY_SYSTEM_META)?;
        let links_meta = *dbs.get(&*PRIMARY_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }

    pub fn cache(reader: &'env R, dbs: &'env DbManager) -> DatabaseResult<Self> {
        let system_meta = *dbs.get(&*CACHE_SYSTEM_META)?;
        let links_meta = *dbs.get(&*CACHE_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for ChainMetaBuf<'env, (), R>
where
    R: Readable,
{
    // TODO find out whether we need link_type.
    fn get_links(&self, base: EntryAddress, tag: Tag) -> DatabaseResult<HashSet<EntryAddress>> {
        // TODO get removes
        // TODO get adds
        let key = LinkKey {
            op: Op::Add,
            base: &base,
            tag,
        };
        let _values = self.links_meta.get(&key.to_key());
        Ok(HashSet::new())
    }
    fn get_crud(&self, _entry_address: EntryAddress) -> DatabaseResult<EntryDhtStatus> {
        unimplemented!()
    }
}

mock! {
    pub ChainMetaBuf
    {
        fn get_links(&self, base: EntryAddress, tag: Tag) -> DatabaseResult<HashSet<EntryAddress>>;
        fn get_crud(&self, entry_address: EntryAddress) -> DatabaseResult<EntryDhtStatus>;
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for MockChainMetaBuf
where
    R: Readable,
{
    fn get_links(&self, base: EntryAddress, tag: Tag) -> DatabaseResult<HashSet<EntryAddress>> {
        self.get_links(base, tag)
    }
    fn get_crud(&self, entry_address: EntryAddress) -> DatabaseResult<EntryDhtStatus> {
        self.get_crud(entry_address)
    }
}
