//! # Cascade
//! ## Retrieve vs Get
//! Get checks CRUD metadata before returning an the data
//! where as retrieve only checks that where the data was found
//! the appropriate validation has been run.

use super::{
    element_buf::ElementBuf,
    metadata::{LinkMetaKey, MetadataBuf, MetadataBufT},
};
use crate::core::workflow::{
    integrate_dht_ops_workflow::integrate_single_metadata,
    produce_dht_ops_workflow::dht_op_light::error::DhtOpConvertResult,
};
use error::CascadeResult;
use fallible_iterator::FallibleIterator;
use holo_hash::{hash_type::AnyDht, AnyDhtHash, EntryHash, HeaderHash};
use holochain_p2p::HolochainP2pCellT;
use holochain_p2p::{
    actor::{GetLinksOptions, GetMetaOptions, GetOptions},
    HolochainP2pCell,
};
use holochain_state::{error::DatabaseResult, fresh_reader, prelude::*};
use holochain_types::{
    dht_op::{produce_op_lights_from_element_group, produce_op_lights_from_elements},
    element::{
        Element, ElementGroup, GetElementResponse, RawGetEntryResponse, SignedHeaderHashed,
        SignedHeaderHashedExt,
    },
    entry::option_entry_hashed,
    link::{GetLinksResponse, WireLinkMetaKey},
    metadata::{EntryDhtStatus, MetadataSet, TimedHeaderHash},
    EntryHashed, HeaderHashed,
};
use holochain_zome_types::header::{CreateLink, DeleteLink};
use holochain_zome_types::{
    element::SignedHeader,
    header::{Delete, Update},
    link::Link,
    metadata::{Details, ElementDetails, EntryDetails},
    Header,
};
use std::convert::TryFrom;
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::TryInto,
};
use tracing::*;
use tracing_futures::Instrument;

#[cfg(test)]
mod authored_test;
#[cfg(test)]
mod network_tests;

#[cfg(all(test, outdated_tests))]
mod test;

pub mod error;

/////////////////
// Helper macros
/////////////////

/// Get an item from an option
/// or return early from the function
macro_rules! ok_or_return {
    ($n:expr) => {
        match $n {
            Some(n) => n,
            None => return Ok(()),
        }
    };
    ($n:expr, $ret:expr) => {
        match $n {
            Some(n) => n,
            None => return Ok($ret),
        }
    };
}

/// Return from a function if
/// an item is found otherwise continue
macro_rules! return_if_ok {
    ($i:expr) => {
        if let Some(i) = $i {
            return Ok(Some(i));
        }
    };
}

/// Search every level that the cascade has been constructed with
macro_rules! search_all {
    ($cascade:expr, $fn:ident, $hash:expr) => {{
        if let Some(db) = $cascade.authored_data.as_ref() {
            return_if_ok!($fn(db, $hash)?)
        }
        if let Some(db) = $cascade.pending_data.as_ref() {
            return_if_ok!($fn(db, $hash)?)
        }
        if let Some(db) = $cascade.integrated_data.as_ref() {
            return_if_ok!($fn(db, $hash)?)
        }
        if let Some(db) = $cascade.rejected_data.as_ref() {
            return_if_ok!($fn(db, $hash)?)
        }
        if let Some(db) = $cascade.cache_data.as_ref() {
            let db = DbPair::from(db);
            return_if_ok!($fn(&db, $hash)?)
        }
        Ok(None)
    }};
}

/// A pair containing an element buf and metadata buf
/// with the same prefix.
/// The default IntegratedPrefix is for databases that don't
/// actually use prefixes (like the cache). In this case we just
/// choose the first one (IntegratedPrefix)
pub struct DbPair<'a, M, P = IntegratedPrefix>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a ElementBuf<P>,
    pub meta: &'a M,
}

pub struct DbPairMut<'a, M, P = IntegratedPrefix>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a mut ElementBuf<P>,
    pub meta: &'a mut M,
}

pub struct Cascade<
    'a,
    Network = HolochainP2pCell,
    MetaVault = MetadataBuf,
    MetaAuthored = MetadataBuf<AuthoredPrefix>,
    MetaCache = MetadataBuf,
    MetaPending = MetadataBuf<PendingPrefix>,
    MetaRejected = MetadataBuf<RejectedPrefix>,
> where
    Network: HolochainP2pCellT,
    MetaVault: MetadataBufT,
    MetaAuthored: MetadataBufT<AuthoredPrefix>,
    MetaPending: MetadataBufT<PendingPrefix>,
    MetaRejected: MetadataBufT<RejectedPrefix>,
    MetaCache: MetadataBufT,
{
    integrated_data: Option<DbPair<'a, MetaVault, IntegratedPrefix>>,
    authored_data: Option<DbPair<'a, MetaAuthored, AuthoredPrefix>>,
    pending_data: Option<DbPair<'a, MetaPending, PendingPrefix>>,
    rejected_data: Option<DbPair<'a, MetaRejected, RejectedPrefix>>,
    cache_data: Option<DbPairMut<'a, MetaCache>>,
    env: Option<EnvironmentRead>,
    network: Option<Network>,
}

#[derive(Debug)]
/// The state of the cascade search
enum Search {
    /// The entry is found and we can stop
    Found(Element),
    /// We haven't found the entry yet and should
    /// continue searching down the cascade
    Continue(HeaderHash),
    /// We haven't found the entry and should
    /// not continue searching down the cascade
    // TODO This information is currently not passed back to
    // the caller however it might be useful.
    NotInCascade,
}

impl<'a, Network, MetaVault, MetaAuthored, MetaCache>
    Cascade<'a, Network, MetaVault, MetaAuthored, MetaCache>
where
    MetaCache: MetadataBufT,
    MetaVault: MetadataBufT,
    MetaAuthored: MetadataBufT<AuthoredPrefix>,
    Network: HolochainP2pCellT,
{
    /// Constructs a [Cascade], for the default use case of
    /// vault + cache + network
    // TODO: Probably should rename this function but want to
    // avoid refactoring
    #[allow(clippy::complexity)]
    pub fn new(
        env: EnvironmentRead,
        element_authored: &'a ElementBuf<AuthoredPrefix>,
        meta_authored: &'a MetaAuthored,
        element_integrated: &'a ElementBuf,
        meta_integrated: &'a MetaVault,
        element_cache: &'a mut ElementBuf,
        meta_cache: &'a mut MetaCache,
        network: Network,
    ) -> Self {
        let authored_data = Some(DbPair {
            element: element_authored,
            meta: meta_authored,
        });
        let integrated_data = Some(DbPair {
            element: element_integrated,
            meta: meta_integrated,
        });
        let cache_data = Some(DbPairMut {
            element: element_cache,
            meta: meta_cache,
        });
        Self {
            env: Some(env),
            network: Some(network),
            pending_data: None,
            rejected_data: None,
            integrated_data,
            authored_data,
            cache_data,
        }
    }
}

impl<'a> Cascade<'a> {
    /// Construct a completely empty cascade
    pub fn empty() -> Self {
        Self {
            integrated_data: None,
            authored_data: None,
            pending_data: None,
            rejected_data: None,
            cache_data: None,
            env: None,
            network: None,
        }
    }
}

impl<'a, Network, MetaVault, MetaAuthored, MetaCache, MetaPending, MetaRejected>
    Cascade<'a, Network, MetaVault, MetaAuthored, MetaCache, MetaPending, MetaRejected>
where
    MetaCache: MetadataBufT,
    MetaVault: MetadataBufT,
    MetaAuthored: MetadataBufT<AuthoredPrefix>,
    MetaPending: MetadataBufT<PendingPrefix>,
    MetaRejected: MetadataBufT<RejectedPrefix>,
    Network: HolochainP2pCellT,
{
    /// Add the integrated [ElementBuf] and [MetadataBuf] to the cascade
    pub fn with_integrated(
        mut self,
        integrated_data: DbPair<'a, MetaVault, IntegratedPrefix>,
    ) -> Self {
        self.env = Some(integrated_data.meta.env().clone());
        self.integrated_data = Some(integrated_data);
        self
    }

    /// Add the integrated [ElementBuf] and [MetadataBuf] to the cascade
    pub fn with_pending(mut self, pending_data: DbPair<'a, MetaPending, PendingPrefix>) -> Self {
        self.env = Some(pending_data.meta.env().clone());
        self.pending_data = Some(pending_data);
        self
    }

    /// Add the authored [ElementBuf] and [MetadataBuf] to the cascade
    pub fn with_authored(
        mut self,
        authored_data: DbPair<'a, MetaAuthored, AuthoredPrefix>,
    ) -> Self {
        self.env = Some(authored_data.meta.env().clone());
        self.authored_data = Some(authored_data);
        self
    }

    /// Add the rejected [ElementBuf] and [MetadataBuf] to the cascade
    pub fn with_rejected(
        mut self,
        rejected_data: DbPair<'a, MetaRejected, RejectedPrefix>,
    ) -> Self {
        self.env = Some(rejected_data.meta.env().clone());
        self.rejected_data = Some(rejected_data);
        self
    }

    /// Add the cache [ElementBuf] and [MetadataBuf] to the cascade
    pub fn with_cache(mut self, cache_data: DbPairMut<'a, MetaCache>) -> Self {
        self.env = Some(cache_data.meta.env().clone());
        self.cache_data = Some(cache_data);
        self
    }

    /// Add the integrated [ElementBuf] and [MetadataBuf] to the cascade
    pub fn with_network<N: HolochainP2pCellT>(
        self,
        network: N,
    ) -> Cascade<'a, N, MetaVault, MetaAuthored, MetaCache, MetaPending, MetaRejected> {
        Cascade {
            integrated_data: self.integrated_data,
            authored_data: self.authored_data,
            pending_data: self.pending_data,
            rejected_data: self.rejected_data,
            cache_data: self.cache_data,
            env: self.env,
            network: Some(network),
        }
    }

    async fn update_stores(&mut self, element: Element) -> CascadeResult<()> {
        let cache_data = ok_or_return!(self.cache_data.as_mut());
        let op_lights = produce_op_lights_from_elements(vec![&element]).await?;
        let (shh, e) = element.into_inner();
        cache_data.element.put(shh, option_entry_hashed(e).await)?;
        for op in op_lights {
            integrate_single_metadata(op, cache_data.element, cache_data.meta)?
        }
        Ok(())
    }

    #[instrument(skip(self, elements))]
    async fn update_stores_with_element_group(
        &mut self,
        elements: ElementGroup<'_>,
    ) -> CascadeResult<()> {
        let cache_data = ok_or_return!(self.cache_data.as_mut());
        let op_lights = produce_op_lights_from_element_group(&elements).await?;
        cache_data.element.put_element_group(elements)?;
        for op in op_lights {
            integrate_single_metadata(op, cache_data.element, cache_data.meta)?
        }
        Ok(())
    }

    async fn fetch_element_via_header(
        &mut self,
        hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let results = network.get(hash.into(), options).await?;
        // Search through the returns for the first delete
        for response in results.into_iter() {
            match response {
                // Has header
                GetElementResponse::GetHeader(Some(we)) => {
                    let (element, delete) = we.into_element_and_delete().await;
                    self.update_stores(element).await?;

                    if let Some(delete) = delete {
                        self.update_stores(delete).await?;
                    }
                }
                // Doesn't have header but not because it was deleted
                GetElementResponse::GetHeader(None) => (),
                r => {
                    error!(
                        msg = "Got an invalid response to fetch element via header",
                        ?r
                    );
                }
            }
        }
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_element_via_entry(
        &mut self,
        hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let results = network
            .get(hash.clone().into(), options.clone())
            .instrument(debug_span!("fetch_element_via_entry::network_get"))
            .await?;

        for response in results {
            match response {
                GetElementResponse::GetEntryFull(Some(raw)) => {
                    let RawGetEntryResponse {
                        live_headers,
                        deletes,
                        entry,
                        entry_type,
                        updates,
                    } = *raw;
                    let elements =
                        ElementGroup::from_wire_elements(live_headers, entry_type, entry).await?;
                    let entry_hash = elements.entry_hash().clone();
                    self.update_stores_with_element_group(elements).await?;
                    for delete in deletes {
                        let element = delete.into_element().await;
                        self.update_stores(element).await?;
                    }
                    for update in updates {
                        let element = update.into_element(entry_hash.clone()).await;
                        self.update_stores(element).await?;
                    }
                }
                // Authority didn't have any headers for this entry
                GetElementResponse::GetEntryFull(None) => (),
                r @ GetElementResponse::GetHeader(_) => {
                    error!(
                        msg = "Got an invalid response to fetch element via entry",
                        ?r
                    );
                }
                r => unimplemented!("{:?} is unimplemented for fetching via entry", r),
            }
        }
        Ok(())
    }

    // TODO: Remove when used
    #[allow(dead_code)]
    async fn fetch_meta(
        &mut self,
        basis: AnyDhtHash,
        options: GetMetaOptions,
    ) -> CascadeResult<Vec<MetadataSet>> {
        let network = ok_or_return!(self.network.as_mut(), vec![]);
        Ok(network.get_meta(basis.clone(), options).await?)
    }

    #[instrument(skip(self, options))]
    async fn fetch_links(
        &mut self,
        link_key: WireLinkMetaKey,
        options: GetLinksOptions,
    ) -> CascadeResult<()> {
        debug!("in get links");
        let network = ok_or_return!(self.network.as_mut());
        let results = network.get_links(link_key, options).await?;

        for links in results {
            let GetLinksResponse {
                link_adds,
                link_removes,
            } = links;

            for (link_add, signature) in link_adds {
                debug!(?link_add);
                let element = Element::new(
                    SignedHeaderHashed::from_content_sync(SignedHeader(link_add.into(), signature)),
                    None,
                );
                self.update_stores(element).await?;
            }
            for (link_remove, signature) in link_removes {
                debug!(?link_remove);
                let element = Element::new(
                    SignedHeaderHashed::from_content_sync(SignedHeader(
                        link_remove.into(),
                        signature,
                    )),
                    None,
                );
                self.update_stores(element).await?;
            }
        }
        Ok(())
    }

    /// Get the element from any databases that the Cascade has been constructed with
    fn get_element_local_raw(&self, hash: &HeaderHash) -> CascadeResult<Option<Element>> {
        // It's a little tricky to call a function on every db.
        // Closures don't work so an inline generic function is needed.
        fn get_element<P: PrefixType, M: MetadataBufT<P>>(
            db: &DbPair<M, P>,
            hash: &HeaderHash,
        ) -> CascadeResult<Option<Element>> {
            Ok(db.element.get_element(hash)?)
        }
        search_all!(self, get_element, hash)
    }

    /// Gets the first element we can find for this entry locally
    fn get_element_local_raw_via_entry(&self, hash: &EntryHash) -> CascadeResult<Option<Element>> {
        fn get_entry<P: PrefixType, M: MetadataBufT<P>>(
            db: &DbPair<M, P>,
            hash: &EntryHash,
        ) -> CascadeResult<Option<Element>> {
            fresh_reader!(db.meta.env(), |r| {
                let mut iter = db.meta.get_headers(&r, hash.clone())?;
                while let Some(h) = iter.next()? {
                    return_if_ok!(db.element.get_element(&h.header_hash)?)
                }
                Ok(None)
            })
        }
        search_all!(self, get_entry, hash)
    }

    /// Get the entry from any databases that the Cascade has been constructed with
    fn get_entry_local_raw(&self, hash: &EntryHash) -> CascadeResult<Option<EntryHashed>> {
        fn get_entry<P: PrefixType, M: MetadataBufT<P>>(
            db: &DbPair<M, P>,
            hash: &EntryHash,
        ) -> CascadeResult<Option<EntryHashed>> {
            Ok(db.element.get_entry(hash)?)
        }
        search_all!(self, get_entry, hash)
    }

    /// Get the header from any databases that the Cascade has been constructed with
    fn get_header_local_raw(&self, hash: &HeaderHash) -> CascadeResult<Option<HeaderHashed>> {
        Ok(self
            .get_header_local_raw_with_sig(hash)?
            .map(|h| h.into_header_and_signature().0))
    }

    fn get_header_local_raw_with_sig(
        &self,
        hash: &HeaderHash,
    ) -> CascadeResult<Option<SignedHeaderHashed>> {
        fn get_header<P: PrefixType, M: MetadataBufT<P>>(
            db: &DbPair<M, P>,
            hash: &HeaderHash,
        ) -> CascadeResult<Option<SignedHeaderHashed>> {
            Ok(db.element.get_header(hash)?)
        }
        search_all!(self, get_header, hash)
    }

    fn render_headers<T, F>(&self, headers: Vec<TimedHeaderHash>, f: F) -> CascadeResult<Vec<T>>
    where
        F: Fn(Header) -> DhtOpConvertResult<T>,
    {
        let mut result = Vec::with_capacity(headers.len());
        for h in headers {
            let hash = h.header_hash;
            let h = self.get_header_local_raw(&hash)?;
            match h {
                Some(h) => result.push(f(HeaderHashed::into_content(h))?),
                None => continue,
            }
        }
        Ok(result)
    }

    async fn create_entry_details(&self, hash: EntryHash) -> CascadeResult<Option<EntryDetails>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        match self.get_entry_local_raw(&hash)? {
            Some(entry) => fresh_reader!(env, |r| {
                let entry_dht_status = cache_data.meta.get_dht_status(&r, &hash)?;
                let headers = cache_data
                    .meta
                    .get_headers(&r, hash.clone())?
                    .collect::<Vec<_>>()?;
                let headers = self.render_headers(headers, Ok)?;
                let deletes = cache_data
                    .meta
                    .get_deletes_on_entry(&r, hash.clone())?
                    .collect::<Vec<_>>()?;
                let deletes = self.render_headers(deletes, |h| Ok(Delete::try_from(h)?))?;
                let updates = cache_data
                    .meta
                    .get_updates(&r, hash.into())?
                    .collect::<Vec<_>>()?;
                let updates = self.render_headers(updates, |h| Ok(Update::try_from(h)?))?;
                Ok(Some(EntryDetails {
                    entry: entry.into_content(),
                    headers,
                    deletes,
                    updates,
                    entry_dht_status,
                }))
            }),
            None => Ok(None),
        }
    }

    fn create_element_details(&self, hash: HeaderHash) -> CascadeResult<Option<ElementDetails>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        match self.get_element_local_raw(&hash)? {
            Some(element) => {
                let hash = element.header_address().clone();
                let deletes = fresh_reader!(env, |r| cache_data
                    .meta
                    .get_deletes_on_header(&r, hash)?
                    .collect::<Vec<_>>())?;
                let deletes = self.render_headers(deletes, |h| Ok(Delete::try_from(h)?))?;
                Ok(Some(ElementDetails { element, deletes }))
            }
            None => Ok(None),
        }
    }

    /// Check if this hash has been validated.
    /// Elements can end up in the cache or integrated table because
    /// they were gossiped to you or you authored them.
    /// If you care about the hash you are using being valid in the same
    /// way as if you got it from the StoreElement authority you can use
    /// this function to verify that constraint.
    ///
    /// An example of how this could go wrong is you do a get for a HeaderHash
    /// where you are the authority for the RegisterAgentActivity for this header.
    /// That hash is in your integrated db so you find it but the element has failed
    /// app validation. The header appears valid even though it isn't because as a
    /// RegisterAgentActivity authority you haven't run app validation.
    pub fn valid_header(&self, hash: &HeaderHash) -> CascadeResult<bool> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), false);
        let integrated_data = ok_or_return!(self.integrated_data.as_ref(), false);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), false);
        Ok(integrated_data.meta.has_registered_store_element(&hash)?
            || cache_data.meta.has_registered_store_element(&hash)?
            || authored_data.meta.has_registered_store_element(&hash)?)
    }

    /// Same as valid_header but checks for StoreEntry validation
    /// See valid_header for details
    pub fn valid_entry(&self, hash: &EntryHash) -> CascadeResult<bool> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), false);
        let integrated_data = ok_or_return!(self.integrated_data.as_ref(), false);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), false);
        if cache_data.meta.has_any_registered_store_entry(hash)? {
            // Found a entry header in the cache
            return Ok(true);
        }
        if authored_data.meta.has_any_registered_store_entry(hash)? {
            // Found a entry header in the authored store
            return Ok(true);
        }
        if integrated_data.meta.has_any_registered_store_entry(hash)? {
            // Found a entry header in the vault
            return Ok(true);
        }
        Ok(false)
    }

    /// Check if we have a valid reason to return an element from the cascade
    /// See valid_header for details
    pub fn valid_element(
        &self,
        header_hash: &HeaderHash,
        entry_hash: Option<&EntryHash>,
    ) -> CascadeResult<bool> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), false);
        let integrated_data = ok_or_return!(self.integrated_data.as_ref(), false);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), false);
        if self.valid_header(&header_hash)? {
            return Ok(true);
        }
        if let Some(eh) = entry_hash {
            if cache_data
                .meta
                .has_registered_store_entry(eh, header_hash)?
            {
                // Found a entry header in the cache
                return Ok(true);
            }
            if authored_data
                .meta
                .has_registered_store_entry(eh, header_hash)?
            {
                // Found a entry header in the authored
                return Ok(true);
            }
            if integrated_data
                .meta
                .has_registered_store_entry(eh, header_hash)?
            {
                // Found a entry header in the vault
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[instrument(skip(self, options))]
    pub async fn get_entry_details(
        &mut self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<EntryDetails>> {
        debug!("in get entry details");
        // Update the cache from the network
        self.fetch_element_via_entry(entry_hash.clone(), options.clone())
            .await?;

        // Get the entry and metadata
        self.create_entry_details(entry_hash).await
    }

    fn get_old_live_element<P: PrefixType, M: MetadataBufT<P>>(
        &self,
        entry_hash: &EntryHash,
        db: &DbPair<M, P>,
    ) -> CascadeResult<Option<Search>> {
        let env = ok_or_return!(self.env.as_ref(), None);
        fresh_reader!(env, |r| {
            match db.meta.get_dht_status(&r, entry_hash)? {
                EntryDhtStatus::Live => {
                    let oldest_live_header = db
                        .meta
                        .get_headers(&r, entry_hash.clone())?
                        .filter_map(|header| {
                            if db
                                .meta
                                .get_deletes_on_header(&r, header.header_hash.clone())?
                                .next()?
                                .is_none()
                            {
                                Ok(Some(header))
                            } else {
                                Ok(None)
                            }
                        })
                        .min()?
                        .expect("Status is live but no headers?");

                    // We have an oldest live header now get the element
                    CascadeResult::Ok(Some(
                        self.get_element_local_raw(&oldest_live_header.header_hash)?
                            .map(Search::Found)
                            // It's not local so check the network
                            .unwrap_or(Search::Continue(oldest_live_header.header_hash)),
                    ))
                }
                EntryDhtStatus::Dead
                | EntryDhtStatus::Pending
                | EntryDhtStatus::Rejected
                | EntryDhtStatus::Abandoned
                | EntryDhtStatus::Conflict
                | EntryDhtStatus::Withdrawn
                | EntryDhtStatus::Purged => CascadeResult::Ok(None),
            }
        })
    }

    #[instrument(skip(self, options))]
    /// Returns the oldest live [Element] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    pub async fn dht_get_entry(
        &mut self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        debug!("in get entry");
        // Update the cache from the network
        self.fetch_element_via_entry(entry_hash.clone(), options.clone())
            .await?;

        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), None);
        // Meta Cache
        let oldest_live_element =
            match self.get_old_live_element(&entry_hash, &DbPair::from(cache_data))? {
                // Look for the element data in authored
                Some(Search::Continue(oldest_live_header)) => {
                    match authored_data.element.get_element(&oldest_live_header)? {
                        Some(element) => Search::Found(element),
                        None => Search::Continue(oldest_live_header),
                    }
                }
                Some(s) => s,
                // Search the authored store
                None => self
                    .get_old_live_element(&entry_hash, authored_data)?
                    .unwrap_or(Search::NotInCascade),
            };

        // Network
        match oldest_live_element {
            Search::Found(element) => Ok(Some(element)),
            Search::Continue(oldest_live_header) => {
                self.dht_get_header(oldest_live_header, options).await
            }
            Search::NotInCascade => Ok(None),
        }
    }

    #[instrument(skip(self, options))]
    pub async fn get_header_details(
        &mut self,
        header_hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<Option<ElementDetails>> {
        debug!("in get header details");
        // Network
        self.fetch_element_via_header(header_hash.clone(), options)
            .await?;

        // Get the element and the metadata
        self.create_element_details(header_hash)
    }

    #[instrument(skip(self, options))]
    /// Returns the [Element] for this [HeaderHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    /// _Note: Deleted headers are a tombstone set_
    pub async fn dht_get_header(
        &mut self,
        header_hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let integrated_data = ok_or_return!(self.integrated_data.as_ref(), None);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        debug!("in get header");
        let found_local_delete = fresh_reader!(env, |r| {
            let in_cache = || {
                DatabaseResult::Ok({
                    cache_data
                        .meta
                        .get_deletes_on_header(&r, header_hash.clone())?
                        .next()?
                        .is_some()
                })
            };
            let in_authored = || {
                DatabaseResult::Ok({
                    authored_data
                        .meta
                        .get_deletes_on_header(&r, header_hash.clone())?
                        .next()?
                        .is_some()
                })
            };
            let in_vault = || {
                DatabaseResult::Ok({
                    integrated_data
                        .meta
                        .get_deletes_on_header(&r, header_hash.clone())?
                        .next()?
                        .is_some()
                })
            };
            DatabaseResult::Ok(in_cache()? || in_authored()? || in_vault()?)
        })?;
        if found_local_delete {
            return Ok(None);
        }
        // Network
        self.fetch_element_via_header(header_hash.clone(), options)
            .await?;

        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        fresh_reader!(env, |r| {
            // Check if header is alive after fetch
            let is_live = cache_data
                .meta
                .get_deletes_on_header(&r, header_hash.clone())?
                .next()?
                .is_none();

            if is_live {
                self.get_element_local_raw(&header_hash)
            } else {
                Ok(None)
            }
        })
    }

    /// Get the entry from the dht regardless of metadata or validation status.
    /// This call has the opportunity to hit the local cache
    /// and avoid a network call.
    // TODO: This still fetches the full element and metadata.
    // Need to add a fetch_retrieve_entry that only gets data.
    pub async fn retrieve_entry(
        &mut self,
        hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<EntryHashed>> {
        match self.get_entry_local_raw(&hash)? {
            Some(e) => Ok(Some(e)),
            None => {
                self.fetch_element_via_entry(hash.clone(), options).await?;
                self.get_entry_local_raw(&hash)
            }
        }
    }

    /// Get only the header from the dht regardless of metadata or validation status.
    /// Useful for avoiding getting the Entry if you don't need it.
    /// This call has the opportunity to hit the local cache
    /// and avoid a network call.
    // TODO: This still fetches the full element and metadata.
    // Need to add a fetch_retrieve_header that only gets data.
    pub async fn retrieve_header(
        &mut self,
        hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<Option<SignedHeaderHashed>> {
        match self.get_header_local_raw_with_sig(&hash)? {
            Some(h) => Ok(Some(h)),
            None => {
                self.fetch_element_via_header(hash.clone(), options).await?;
                self.get_header_local_raw_with_sig(&hash)
            }
        }
    }

    /// Get an element from the dht regardless of metadata or validation status.
    /// Useful for checking if data is held.
    /// This call has the opportunity to hit the local cache
    /// and avoid a network call.
    /// Note we still need to return the element as proof they are really
    /// holding it unless we create a byte challenge function.
    // TODO: This still fetches the full element and metadata.
    // Need to add a fetch_retrieve that only gets data.
    pub async fn retrieve(
        &mut self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        match *hash.hash_type() {
            AnyDht::Entry => {
                let hash = hash.into();
                match self.get_element_local_raw_via_entry(&hash)? {
                    Some(e) => Ok(Some(e)),
                    None => {
                        self.fetch_element_via_entry(hash.clone(), options).await?;
                        self.get_element_local_raw_via_entry(&hash)
                    }
                }
            }
            AnyDht::Header => {
                let hash = hash.into();
                match self.get_element_local_raw(&hash)? {
                    Some(e) => Ok(Some(e)),
                    None => {
                        self.fetch_element_via_header(hash.clone(), options).await?;
                        self.get_element_local_raw(&hash)
                    }
                }
            }
        }
    }

    #[instrument(skip(self))]
    /// Updates the cache with the latest network authority data
    /// and returns what is in the cache.
    /// This gives you the latest possible picture of the current dht state.
    /// Data from your zome call is also added to the cache.
    pub async fn dht_get(
        &mut self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        match *hash.hash_type() {
            AnyDht::Entry => self.dht_get_entry(hash.into(), options).await,
            AnyDht::Header => self.dht_get_header(hash.into(), options).await,
        }
    }

    #[instrument(skip(self))]
    pub async fn get_details(
        &mut self,
        hash: AnyDhtHash,
        mut options: GetOptions,
    ) -> CascadeResult<Option<Details>> {
        options.all_live_headers_with_metadata = true;
        match *hash.hash_type() {
            AnyDht::Entry => Ok(self
                .get_entry_details(hash.into(), options)
                .await?
                .map(Details::Entry)),
            AnyDht::Header => Ok(self
                .get_header_details(hash.into(), options)
                .await?
                .map(Details::Element)),
        }
    }

    #[instrument(skip(self, key, options))]
    /// Gets an links from the cas or cache depending on it's metadata
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get_links<'link>(
        &mut self,
        key: &'link LinkMetaKey<'link>,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<Link>> {
        // Update the cache from the network
        self.fetch_links(key.into(), options).await?;

        let cache_data = ok_or_return!(self.cache_data.as_ref(), vec![]);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), vec![]);
        let env = ok_or_return!(self.env.as_ref(), vec![]);
        fresh_reader!(env, |r| {
            // Meta Cache
            // Return any links from the meta cache that don't have removes.
            // TODO: Check for duplicates with a HashSet
            Ok(cache_data
                .meta
                .get_live_links(&r, key)?
                .map(|l| Ok(l.into_link()))
                .chain(
                    authored_data
                        .meta
                        .get_live_links(&r, key)?
                        .map(|l| Ok(l.into_link())),
                )
                .collect()?)
        })
    }

    #[instrument(skip(self, key, options))]
    /// Return all CreateLink headers
    /// and DeleteLink headers ordered by time.
    pub async fn get_link_details<'link>(
        &mut self,
        key: &'link LinkMetaKey<'link>,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<(CreateLink, Vec<DeleteLink>)>> {
        // Update the cache from the network
        self.fetch_links(key.into(), options).await?;

        let cache_data = ok_or_return!(self.cache_data.as_ref(), vec![]);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), vec![]);
        let env = ok_or_return!(self.env.as_ref(), vec![]);
        // Get the links and collect the CreateLink / DeleteLink hashes by time.
        // TODO: Search authored and combine with cache_data
        let links = fresh_reader!(env, |r| {
            cache_data
                .meta
                .get_links_all(&r, key)?
                .map(|link_add| {
                    // Collect the link removes on this link add
                    let link_removes = cache_data
                        .meta
                        .get_link_removes_on_link_add(&r, link_add.link_add_hash.clone())?
                        .collect::<BTreeSet<_>>()?;
                    // Create timed header hash
                    let link_add = TimedHeaderHash {
                        timestamp: link_add.timestamp,
                        header_hash: link_add.link_add_hash,
                    };
                    // Return all link removes with this link add
                    Ok((link_add, link_removes))
                })
                .chain(authored_data.meta.get_links_all(&r, key)?.map(|link_add| {
                    // Collect the link removes on this link add
                    let link_removes = authored_data
                        .meta
                        .get_link_removes_on_link_add(&r, link_add.link_add_hash.clone())?
                        .collect::<BTreeSet<_>>()?;
                    // Create timed header hash
                    let link_add = TimedHeaderHash {
                        timestamp: link_add.timestamp,
                        header_hash: link_add.link_add_hash,
                    };
                    // Return all link removes with this link add
                    Ok((link_add, link_removes))
                }))
                .collect::<BTreeMap<_, _>>()
        })?;
        // Get the headers from the element stores
        let mut result: Vec<(CreateLink, _)> = Vec::with_capacity(links.len());
        for (link_add, link_removes) in links {
            if let Some(link_add) = self.get_element_local_raw(&link_add.header_hash)? {
                let mut r: Vec<DeleteLink> = Vec::with_capacity(link_removes.len());
                for link_remove in link_removes {
                    if let Some(link_remove) =
                        self.get_element_local_raw(&link_remove.header_hash)?
                    {
                        r.push(link_remove.try_into()?);
                    }
                }
                result.push((link_add.try_into()?, r));
            }
        }
        Ok(result)
    }
}

impl<'a, M: MetadataBufT> From<&'a DbPairMut<'a, M>> for DbPair<'a, M> {
    fn from(n: &'a DbPairMut<'a, M>) -> Self {
        Self {
            element: n.element,
            meta: n.meta,
        }
    }
}

#[cfg(test)]
/// Helper function for easily setting up cascades during tests
pub fn test_dbs_and_mocks(
    env: EnvironmentRead,
) -> (
    ElementBuf,
    super::metadata::MockMetadataBuf,
    ElementBuf,
    super::metadata::MockMetadataBuf,
) {
    let cas = ElementBuf::vault(env.clone().into(), true).unwrap();
    let element_cache = ElementBuf::cache(env.clone().into()).unwrap();
    let metadata = super::metadata::MockMetadataBuf::new();
    let metadata_cache = super::metadata::MockMetadataBuf::new();
    (cas, metadata, element_cache, metadata_cache)
}
