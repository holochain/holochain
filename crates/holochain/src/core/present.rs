use holo_hash::EntryHash;
use holochain_state::{
    error::DatabaseResult,
    fresh_reader,
    prelude::JudgedPrefix,
    prelude::{PendingPrefix, PrefixType},
};
use holochain_zome_types::element::Element;

use crate::core::state::cascade::Cascade;

pub use error::*;

use super::{
    state::element_buf::ElementBuf, state::metadata::MetadataBuf, state::metadata::MetadataBufT,
    workflow::sys_validation_workflow::types::Dependency,
};

mod error;

macro_rules! found {
    ($r:expr) => {
        if let Some(r) = $r {
            return Ok(Some(r));
        }
    };
}

pub struct DbPair<'a, P, M>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a ElementBuf<P>,
    pub meta: &'a M,
}

pub trait DataSource {
    fn cascade(&mut self) -> Cascade;
    fn pending(&self) -> DbPair<PendingPrefix, MetadataBuf<PendingPrefix>>;
    fn judged(&self) -> DbPair<JudgedPrefix, MetadataBuf<JudgedPrefix>>;
}

pub async fn retrieve_entry(
    hash: &EntryHash,
    data_source: &mut impl DataSource,
) -> PresentResult<Option<Dependency<Element>>> {
    use Dependency::*;
    found!(retrieve_entry_from(hash, data_source.judged())?.map(Proof));
    found!(retrieve_entry_from(hash, data_source.pending())?.map(PendingValidation));
    // check_holding_entry!(workspace, check_holding_entry, &entry_hash);
    let mut cascade = data_source.cascade();
    let el = cascade
        .retrieve(hash.clone().into(), Default::default())
        .await?;
    Ok(el.map(Claim))
}

fn retrieve_entry_from<P, M>(hash: &EntryHash, dbs: DbPair<P, M>) -> PresentResult<Option<Element>>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    let eh = fresh_reader!(dbs.meta.env(), |r| {
        let eh = dbs
            .meta
            .get_headers(&r, hash.clone())?
            .next()?
            .map(|h| h.header_hash);
        DatabaseResult::Ok(eh)
    })?;
    match eh {
        Some(entry_header) => Ok(dbs.element.get_element(&entry_header)?),
        None => Ok(None),
    }
}
