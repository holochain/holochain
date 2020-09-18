//! # Present Data Searches
//! Checking for data and metadata presence in different
//! locations.
//! Once data / metadata is found the type of dependency is
//! recorded so that consumers of this api can make decisions
//! about what type of dependency they require (e.g. waiting for
//! a PendingValidation dep to validate).
//! This is still a work in progress and will eventually
//! replace the "present.rs" in the sys_validation_workflow.
use holo_hash::{EntryHash, HeaderHash};
use holochain_state::{
    error::DatabaseResult,
    fresh_reader,
    prelude::JudgedPrefix,
    prelude::{PendingPrefix, PrefixType},
};
use holochain_zome_types::element::{Element, SignedHeaderHashed};

use crate::core::state::cascade::Cascade;

pub use error::*;

use super::{
    state::element_buf::ElementBuf, state::metadata::MetadataBuf, state::metadata::MetadataBufT,
    validation::Dependency,
};

mod error;

macro_rules! found {
    ($r:expr) => {
        if let Some(r) = $r {
            return Ok(Some(r));
        }
    };
}

/// A pair containing an element buf and metadata buf
/// with the same prefix.
/// This is useful for when you want to check
/// a level for some data. For example
/// you might want see if an entry exists in the
/// "judged" element buf and the metadata buf.
pub struct DbPair<'a, P, M>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a ElementBuf<P>,
    pub meta: &'a M,
}

/// A source of data that contains different levels.
/// This trait allows different workspaces to provide
/// the source of the data / metadata for the retrieve calls.
/// You can think of this as a superset of the cascade where we want
/// to be able to also check for data in our other local stores.
// TODO: This might need to be broken into sub traits as
// some workspaces don't have all the levels (e.g. IntegratedDhtOpsWorkspace
// doesn't have pending).
pub trait DataSource {
    fn cascade(&mut self) -> Cascade;
    fn pending(&self) -> DbPair<PendingPrefix, MetadataBuf<PendingPrefix>>;
    fn judged(&self) -> DbPair<JudgedPrefix, MetadataBuf<JudgedPrefix>>;
}

/// Retrieve an element via an EntryHash from the judged, pending or cascade.
/// This call will stop at the first found so _can_ avoid network calls.
/// The entry will only be returned if at least the StoreEntry validation
/// has run or will run.
/// A `Dependency::PendingValidation` will be returned in cases where
/// validation has not run (the op hasn't been judged).
/// _Note: This does not mean the entry is "Valid" only that it will be
/// judged.
pub async fn retrieve_entry(
    hash: &EntryHash,
    data_source: &mut impl DataSource,
) -> PresentResult<Option<Dependency<Element>>> {
    use Dependency::*;
    found!(retrieve_entry_from(hash, data_source.judged())?.map(Proof));
    found!(retrieve_entry_from(hash, data_source.pending())?.map(PendingValidation));
    let mut cascade = data_source.cascade();
    let el = cascade.retrieve_entry(hash, Default::default()).await?;
    Ok(el.map(Claim))
}

/// Retrieve an element via a HeaderHash from the judged, pending or cascade.
/// This call will stop at the first found so _can_ avoid network calls.
/// The element will only be returned if at least the StoreEntry or
/// StoreElement validation has run or will run.
/// A `Dependency::PendingValidation` will be returned in cases where
/// validation has not run (the op hasn't been judged).
/// _Note: This does not mean the entry is "Valid" only that it will be
/// judged.
pub async fn retrieve_element(
    hash: &HeaderHash,
    data_source: &mut impl DataSource,
) -> PresentResult<Option<Dependency<Element>>> {
    use Dependency::*;
    found!(retrieve_element_from(hash, data_source.judged())?.map(Proof));
    found!(retrieve_element_from(hash, data_source.pending())?.map(PendingValidation));
    let mut cascade = data_source.cascade();
    let el = cascade
        .retrieve(hash.clone().into(), Default::default())
        .await?;
    Ok(el.map(Claim))
}

/// Retrieve an SignedHeaderHashed from the judged, pending or cascade.
/// This call will stop at the first found so _can_ avoid network calls.
/// The element will only be returned if at least the StoreEntry or
/// StoreElement validation has run or will run.
/// A `Dependency::PendingValidation` will be returned in cases where
/// validation has not run (the op hasn't been judged).
/// _Note: This does not mean the entry is "Valid" only that it will be
/// judged.
pub async fn retrieve_header(
    hash: &HeaderHash,
    data_source: &mut impl DataSource,
) -> PresentResult<Option<Dependency<SignedHeaderHashed>>> {
    use Dependency::*;
    found!(retrieve_header_from(hash, data_source.judged())?.map(Proof));
    found!(retrieve_header_from(hash, data_source.pending())?.map(PendingValidation));
    let mut cascade = data_source.cascade();
    let shh = cascade
        .retrieve_header(hash.clone().into(), Default::default())
        .await?;
    Ok(shh.map(Claim))
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

fn retrieve_element_from<P, M>(
    hash: &HeaderHash,
    dbs: DbPair<P, M>,
) -> PresentResult<Option<Element>>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    match dbs.element.get_element(&hash)? {
        Some(el) => {
            let mut has_op = dbs.meta.has_registered_store_element(hash)?;
            if let Some(eh) = el.header().entry_hash() {
                has_op = has_op || dbs.meta.has_registered_store_entry(eh, hash)?;
            }
            if has_op {
                Ok(Some(el))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

fn retrieve_header_from<P, M>(
    hash: &HeaderHash,
    dbs: DbPair<P, M>,
) -> PresentResult<Option<SignedHeaderHashed>>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    match dbs.element.get_header(&hash)? {
        Some(shh) => {
            let mut has_op = dbs.meta.has_registered_store_element(hash)?;
            if let Some(eh) = shh.header().entry_hash() {
                has_op = has_op || dbs.meta.has_registered_store_entry(eh, hash)?;
            }
            if has_op {
                Ok(Some(shh))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}
