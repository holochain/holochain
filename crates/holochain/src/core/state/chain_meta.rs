use std::collections::HashSet;
use sx_state::{
    buffer::{BufMultiVal, KvvBuf},
    db::{CACHE_LINKS_META, CACHE_SYSTEM_META, PRIMARY_LINKS_META, PRIMARY_SYSTEM_META},
    error::DatabaseResult,
    prelude::*,
};
use sx_types::persistence::cas::content::Address;

type Tag = String;

enum Op {
    Add,
    Remove,
}

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

pub struct ChainMetaBuf<'env, V, R = Reader<'env>>
where
    V: BufMultiVal,
    R: Readable,
{
    system_meta: KvvBuf<'env, Vec<u8>, V, R>,
    links_meta: KvvBuf<'env, Vec<u8>, V, R>,
}

impl<'env, V, R> ChainMetaBuf<'env, V, R>
where
    V: BufMultiVal,
    R: Readable,
{
    pub fn new(
        reader: &'env R,
        system_meta: MultiStore,
        links_meta: MultiStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            system_meta: KvvBuf::new(reader, system_meta)?,
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
    /// TODO find out whether we need link_type.
    pub fn get_links(&self, base: &Address, tag: Tag) -> DatabaseResult<HashSet<Address>> {
        // TODO get removes
        // TODO get adds
        let key = LinkKey {
            op: Op::Add,
            base,
            tag,
        };
        let values = self.links_meta.get(&key.to_key());
        unimplemented!()
    }
}
