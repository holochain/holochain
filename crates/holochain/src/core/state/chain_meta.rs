#![allow(clippy::ptr_arg)]
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::KvvBuf,
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::DatabaseResult,
    prelude::*,
};
use holochain_types::{composite_hash::EntryAddress, shims::*};
use mockall::mock;
use std::collections::HashSet;
use std::convert::TryInto;
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
        // Possibly FIXME if this expect is actually not true
        let sb: SerializedBytes = self
            .base
            .try_into()
            .expect("entry addresses don't have the unserialize problem");
        let mut vec: Vec<u8> = sb.bytes().to_vec();
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
    fn get_crud(&self, entry_hash: EntryAddress) -> DatabaseResult<EntryDhtStatus>;
}
pub struct ChainMetaBuf<'env, R = Reader<'env>>
where
    R: Readable,
{
    _system_meta: KvvBuf<'env, Vec<u8>, SysMetaVal, R>,
    links_meta: KvvBuf<'env, Vec<u8>, LinkMetaVal, R>,
}

impl<'env, R> ChainMetaBuf<'env, R>
where
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
    pub fn primary(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*PRIMARY_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*PRIMARY_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }

    pub fn cache(reader: &'env R, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*CACHE_LINKS_META)?;
        Self::new(reader, system_meta, links_meta)
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for ChainMetaBuf<'env, R>
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
    fn get_crud(&self, _entry_hash: EntryAddress) -> DatabaseResult<EntryDhtStatus> {
        unimplemented!()
    }
}

mock! {
    pub ChainMetaBuf
    {
        fn get_links(&self, base: EntryAddress, tag: Tag) -> DatabaseResult<HashSet<EntryAddress>>;
        fn get_crud(&self, entry_hash: EntryAddress) -> DatabaseResult<EntryDhtStatus>;
    }
}

impl<'env, R> ChainMetaBufT<'env, R> for MockChainMetaBuf
where
    R: Readable,
{
    fn get_links(&self, base: EntryAddress, tag: Tag) -> DatabaseResult<HashSet<EntryAddress>> {
        self.get_links(base, tag)
    }
    fn get_crud(&self, entry_hash: EntryAddress) -> DatabaseResult<EntryDhtStatus> {
        self.get_crud(entry_hash)
    }
}
