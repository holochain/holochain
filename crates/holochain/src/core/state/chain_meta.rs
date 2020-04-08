#![allow(clippy::ptr_arg)]
use mockall::mock;
use std::collections::HashSet;
use std::fmt::Debug;
use sx_state::{
    buffer::{BufMultiVal, KvvBuf},
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::DatabaseResult,
    prelude::*,
};
use sx_types::persistence::cas::content::Address;

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
    base: &'a Address,
    op: Op,
    tag: Tag,
}

impl<'a> LinkKey<'a> {
    fn to_key(&self) -> Vec<u8> {
        let mut vec: Vec<u8> = self.base.as_ref().into();
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
    fn get_links(&self, base: &Address, tag: Tag) -> DatabaseResult<HashSet<Address>>;
    fn get_crud(&self, address: &Address) -> DatabaseResult<EntryDhtStatus>;
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
    fn get_links(&self, base: &Address, tag: Tag) -> DatabaseResult<HashSet<Address>> {
        // TODO get removes
        // TODO get adds
        let key = LinkKey {
            op: Op::Add,
            base,
            tag,
        };
        let _values = self.links_meta.get(&key.to_key());
        Ok(HashSet::new())
    }
    fn get_crud(&self, _address: &Address) -> DatabaseResult<EntryDhtStatus> {
        unimplemented!()
    }
}

mock! {
    pub ChainMetaBuf
    {
        fn get_links(&self, base: &Address, tag: Tag) -> DatabaseResult<HashSet<Address>>;
        fn get_crud(&self, address: &Address) -> DatabaseResult<EntryDhtStatus>;
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for MockChainMetaBuf
where
    R: Readable,
{
    fn get_links(&self, base: &Address, tag: Tag) -> DatabaseResult<HashSet<Address>> {
        self.get_links(base, tag)
    }
    fn get_crud(&self, address: &Address) -> DatabaseResult<EntryDhtStatus> {
        self.get_crud(address)
    }
}
