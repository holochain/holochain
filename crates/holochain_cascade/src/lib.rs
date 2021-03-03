//! # Cascade
//! ## Retrieve vs Get
//! Get checks CRUD metadata before returning an the data
//! where as retrieve only checks that where the data was found
//! the appropriate validation has been run.

#![allow(deprecated)]

use either::Either;
use error::{AuthorityDataError, CascadeResult};
use fallible_iterator::FallibleIterator;
use holo_hash::hash_type::AnyDht;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetMetaOptions;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::fresh_reader;
use holochain_sqlite::prelude::*;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use tracing::*;
use tracing_futures::Instrument;

pub mod authority;
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
#[derive(derive_more::Constructor)]
pub struct DbPair<'a, M, P = IntegratedPrefix>
where
    P: PrefixType,
    M: MetadataBufT<P>,
{
    pub element: &'a ElementBuf<P>,
    pub meta: &'a M,
}

#[derive(derive_more::Constructor)]
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
    Network: HolochainP2pCellT + Clone,
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
    env: Option<DbRead>,
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

impl<'a, Network, MetaVault, MetaAuthored, MetaCache, MetaPending, MetaRejected>
    Cascade<'a, Network, MetaVault, MetaAuthored, MetaCache, MetaPending, MetaRejected>
where
    MetaCache: MetadataBufT,
    MetaVault: MetadataBufT,
    MetaAuthored: MetadataBufT<AuthoredPrefix>,
    MetaPending: MetadataBufT<PendingPrefix>,
    MetaRejected: MetadataBufT<RejectedPrefix>,
    Network: HolochainP2pCellT + Clone + 'static + Send,
{
    /// Constructs a [Cascade], for the default use case of
    /// vault + cache + network
    // TODO: Probably should rename this function but want to
    // avoid refactoring
    #[allow(clippy::complexity)]
    pub fn new(
        env: DbRead,
        element_authored: &'a ElementBuf<AuthoredPrefix>,
        meta_authored: &'a MetaAuthored,
        element_integrated: &'a ElementBuf,
        meta_integrated: &'a MetaVault,
        element_rejected: &'a ElementBuf<RejectedPrefix>,
        meta_rejected: &'a MetaRejected,
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
        let rejected_data = Some(DbPair {
            element: element_rejected,
            meta: meta_rejected,
        });
        let cache_data = Some(DbPairMut {
            element: element_cache,
            meta: meta_cache,
        });
        Self {
            env: Some(env),
            network: Some(network),
            pending_data: None,
            rejected_data,
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
    Network: HolochainP2pCellT + Clone + 'static + Send,
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
    pub fn with_network<N: HolochainP2pCellT + Clone>(
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

    /// Put a header into the cache when receiving it from a `get_agent_activity` call.
    /// We can't produce all the ops because we don't have the entry.
    async fn update_agent_activity_stores(
        &mut self,
        agent_activity: AgentActivityResponse,
    ) -> CascadeResult<()> {
        let cache_data = ok_or_return!(self.cache_data.as_mut());
        let AgentActivityResponse {
            agent,
            // Cache the chain status in the metadata
            // Any invalid overwrites valid.
            // The earlier chain issue overwrites later.
            status,
            // Cache the highest observed
            // Highest overwrites lower observed.
            // Same seq number are combined to show a fork
            highest_observed,
            valid_activity,
            rejected_activity,
            ..
        } = agent_activity;
        match valid_activity {
            ChainItems::Full(headers) => {
                let hashes = headers
                    .into_iter()
                    .map(|shh| (shh.header().header_seq(), shh.header_address().clone()));
                cache_data.meta.register_activity_sequence(
                    &agent,
                    hashes,
                    ValidationStatus::Valid,
                )?;
            }
            ChainItems::Hashes(hashes) => {
                let hashes = hashes.into_iter();
                cache_data.meta.register_activity_sequence(
                    &agent,
                    hashes,
                    ValidationStatus::Valid,
                )?;
            }
            ChainItems::NotRequested => {}
        };
        match rejected_activity {
            ChainItems::Full(headers) => {
                let hashes = headers
                    .into_iter()
                    .map(|shh| (shh.header().header_seq(), shh.header_address().clone()));
                cache_data.meta.register_activity_sequence(
                    &agent,
                    hashes,
                    ValidationStatus::Rejected,
                )?;
            }
            ChainItems::Hashes(hashes) => {
                let hashes = hashes.into_iter();
                cache_data.meta.register_activity_sequence(
                    &agent,
                    hashes,
                    ValidationStatus::Rejected,
                )?;
            }
            ChainItems::NotRequested => {}
        };
        match &status {
            ChainStatus::Empty => {}
            ChainStatus::Valid(_) | ChainStatus::Forked(_) | ChainStatus::Invalid(_) => {
                cache_data.meta.register_activity_status(&agent, status)?;
            }
        }
        if let Some(highest_observed) = highest_observed {
            cache_data
                .meta
                .register_activity_observed(&agent, highest_observed)?;
        }
        Ok(())
    }

    fn update_stores(&mut self, element_status: ElementStatus) -> CascadeResult<()> {
        let cache_data = ok_or_return!(self.cache_data.as_mut());
        let ElementStatus { element, status } = element_status;
        let op_lights = produce_op_lights_from_elements(vec![&element])?;
        let (shh, e) = element.into_inner();
        cache_data
            .meta
            .register_validation_status(shh.header_address().clone(), status);
        cache_data.element.put(shh, option_entry_hashed(e))?;
        for op in op_lights {
            integrate_single_metadata(op, cache_data.element, cache_data.meta)?
        }
        Ok(())
    }

    // HACK: This is dumb but correct and will be easily
    // avoided with indexing.
    // This just the fastest way to implement getting the integrated
    // data into the cache. Basically a short circuit network call
    // that only makes sense for full sharding.
    fn update_cache_from_integrated(
        &mut self,
        hash: AnyDhtHash,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        if self.cache_data.is_none() {
            return Ok(());
        }
        let env = ok_or_return!(self.env.clone());
        match *hash.hash_type() {
            AnyDht::Entry => {
                let response =
                    authority::handle_get_entry(env.into(), hash.into(), (&options).into())?;
                self.put_entry_in_cache(response)?;
            }
            AnyDht::Header => {
                let response = authority::handle_get_element(env.into(), hash.into())?;
                self.put_element_in_cache(response)?;
            }
        }
        Ok(())
    }

    // HACK: This is dumb but correct and will be easily
    // avoided with indexing.
    // This just the fastest way to implement getting the integrated
    // data into the cache. Basically a short circuit network call
    // that only makes sense for full sharding.
    fn update_link_cache_from_integrated<'link>(
        &mut self,
        key: &'link LinkMetaKey<'link>,
        options: GetLinksOptions,
    ) -> CascadeResult<()> {
        if self.cache_data.is_none() {
            return Ok(());
        }
        let env = ok_or_return!(self.env.clone());
        let response = authority::handle_get_links(env, key.into(), (&options).into())?;
        self.put_link_in_cache(response)?;
        Ok(())
    }

    #[instrument(skip(self, elements))]
    fn update_stores_with_element_group(
        &mut self,
        elements: ElementGroup<'_>,
    ) -> CascadeResult<()> {
        let cache_data = ok_or_return!(self.cache_data.as_mut());
        let op_lights = produce_op_lights_from_element_group(&elements)?;

        // Register the valid hashes
        for hash in elements.valid_hashes().cloned() {
            cache_data
                .meta
                .register_validation_status(hash, ValidationStatus::Valid);
        }
        // Register the rejected hashes
        for hash in elements.valid_hashes().cloned() {
            cache_data
                .meta
                .register_validation_status(hash, ValidationStatus::Valid);
        }

        cache_data.element.put_element_group(elements)?;
        for op in op_lights {
            integrate_single_metadata(op, cache_data.element, cache_data.meta)?
        }
        Ok(())
    }

    fn put_element_in_cache(&mut self, response: GetElementResponse) -> CascadeResult<()> {
        match response {
            // Has header
            GetElementResponse::GetHeader(Some(we)) => {
                let (element_status, deletes, updates) = we.into_parts();
                self.update_stores(element_status)?;

                for delete in deletes {
                    self.update_stores(delete)?;
                }

                for update in updates {
                    self.update_stores(update)?;
                }
            }
            // Doesn't have header but not because it was deleted
            GetElementResponse::GetHeader(None) => {}
            r => {
                error!(
                    msg = "Got an invalid response to fetch element via header",
                    ?r
                );
            }
        }
        Ok(())
    }

    #[instrument(skip(self, hashes, options))]
    /// Exactly the same as fetch_elements_via_entry
    /// except the network is cloned and a task is spawned
    /// for each entry.
    async fn fetch_elements_via_header_parallel<I: IntoIterator<Item = HeaderHash>>(
        &mut self,
        hashes: I,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        // Network needs mut access for calls which we can't share across
        // threads so we need to clone.
        let network = ok_or_return!(self.network.clone());

        // Spawn a task to run in parallel for each entry.
        // This works because we don't need to use self and therefor
        // don't need to share the &mut to our databases across threads.
        let tasks = hashes.into_iter().map(|hash| {
            tokio::task::spawn({
                let mut network = network.clone();
                let options = options.clone();
                async move {
                    network
                        .get(hash.clone().into(), options)
                        .instrument(debug_span!("fetch_element_via_entry::network_get"))
                        .await
                }
            })
        });

        // try waiting on all the gets but exit if any fail
        let all_responses = futures::future::try_join_all(tasks).await?;

        // Put the data into the cache from every authority that responded
        for responses in all_responses {
            for response in responses? {
                self.put_element_in_cache(response)?;
            }
        }
        Ok(())
    }

    async fn fetch_element_via_header(
        &mut self,
        hash: HeaderHash,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let results = network.get(hash.into(), options).await?;
        // Search through the returns for the first delete
        for response in results.into_iter() {
            self.put_element_in_cache(response)?;
        }
        Ok(())
    }

    fn put_entry_in_cache(&mut self, response: GetElementResponse) -> CascadeResult<()> {
        match response {
            GetElementResponse::GetEntryFull(Some(raw)) => {
                let RawGetEntryResponse {
                    live_headers,
                    deletes,
                    entry,
                    entry_type,
                    updates,
                } = *raw;
                let entry_hash = if !live_headers.is_empty() {
                    let elements =
                        ElementGroup::from_wire_elements(live_headers, entry_type, entry)?;
                    let entry_hash = elements.entry_hash().clone();
                    self.update_stores_with_element_group(elements)?;
                    entry_hash
                } else {
                    EntryHash::with_data_sync(&entry)
                };
                for delete in deletes {
                    let element_status = delete.into_element_status();
                    self.update_stores(element_status)?;
                }
                for update in updates {
                    let element_status = update.into_element_status(entry_hash.clone());
                    self.update_stores(element_status)?;
                }
            }
            // Authority didn't have any headers for this entry
            GetElementResponse::GetEntryFull(None) => {}
            r @ GetElementResponse::GetHeader(_) => {
                error!(
                    msg = "Got an invalid response to fetch element via entry",
                    ?r
                );
            }
            r => unimplemented!("{:?} is unimplemented for fetching via entry", r),
        }
        Ok(())
    }

    #[instrument(skip(self, hashes, options))]
    /// Exactly the same as fetch_elements_via_entry
    /// except the network is cloned and a task is spawned
    /// for each entry.
    async fn fetch_elements_via_entry_parallel<I: IntoIterator<Item = EntryHash>>(
        &mut self,
        hashes: I,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        // Network needs mut access for calls which we can't share across
        // threads so we need to clone.
        let network = ok_or_return!(self.network.clone());

        // Spawn a task to run in parallel for each entry.
        // This works because we don't need to use self and therefor
        // don't need to share the &mut to our databases across threads.
        let tasks = hashes.into_iter().map(|hash| {
            tokio::task::spawn({
                let mut network = network.clone();
                let options = options.clone();
                async move {
                    network
                        .get(hash.clone().into(), options)
                        .instrument(debug_span!("fetch_element_via_entry::network_get"))
                        .await
                }
            })
        });

        // try waiting on all the gets but exit if any fail
        let all_responses = futures::future::try_join_all(tasks).await?;

        // Put the data into the cache from every authority that responded
        for responses in all_responses {
            for response in responses? {
                self.put_entry_in_cache(response)?;
            }
        }
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_element_via_entry(
        &mut self,
        hash: EntryHash,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let results = network
            .get(hash.clone().into(), options.clone())
            .instrument(debug_span!("fetch_element_via_entry::network_get"))
            .await?;

        for response in results {
            self.put_entry_in_cache(response)?;
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

    fn put_link_in_cache(&mut self, response: GetLinksResponse) -> CascadeResult<()> {
        let GetLinksResponse {
            link_adds,
            link_removes,
        } = response;

        for (link_add, signature) in link_adds {
            debug!(?link_add);
            let element = Element::new(
                SignedHeaderHashed::from_content_sync(SignedHeader(link_add.into(), signature)),
                None,
            );
            // TODO: Assuming links are also valid headers.
            // We will need to prove this is the case in the future.
            self.update_stores(ElementStatus::new(element, ValidationStatus::Valid))?;
        }
        for (link_remove, signature) in link_removes {
            debug!(?link_remove);
            let element = Element::new(
                SignedHeaderHashed::from_content_sync(SignedHeader(link_remove.into(), signature)),
                None,
            );
            // TODO: Assuming links are also valid headers.
            // We will need to prove this is the case in the future.
            self.update_stores(ElementStatus::new(element, ValidationStatus::Valid))?;
        }
        Ok(())
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

        for response in results {
            self.put_link_in_cache(response)?;
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
                let mut iter = db.meta.get_all_headers(&r, hash.clone())?;
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

    fn render_headers<F>(
        &self,
        headers: impl IntoIterator<Item = TimedHeaderHash>,
        f: F,
    ) -> CascadeResult<Vec<SignedHeaderHashed>>
    where
        F: Fn(HeaderType) -> bool,
    {
        headers
            .into_iter()
            .filter_map(|h| {
                let hash = h.header_hash;
                let h = match self.get_header_local_raw_with_sig(&hash) {
                    Ok(r) => r,
                    Err(e) => return Some(Err(e)),
                };
                match h {
                    Some(h) => {
                        // Check the header type is correct
                        if f(h.header().header_type()) {
                            Some(Ok(h))
                        } else {
                            None
                        }
                    }
                    None => None,
                }
            })
            .collect()
    }

    /// Compute the [EntryDhtStatus] for these headers
    /// from the combined perspective of the cache and
    /// the authored store
    fn compute_entry_dht_status(
        headers: &BTreeSet<TimedHeaderHash>,
        cache_data: &DbPairMut<'a, MetaCache>,
        authored_data: &DbPair<'a, MetaAuthored, AuthoredPrefix>,
        env: &DbRead,
    ) -> CascadeResult<EntryDhtStatus> {
        fresh_reader!(env, |r| {
            for thh in headers {
                // If we can find any header that has no
                // deletes in either store then the entry is live
                if cache_data
                    .meta
                    .get_deletes_on_header(&r, thh.header_hash.clone())?
                    .next()?
                    .is_none()
                    && authored_data
                        .meta
                        .get_deletes_on_header(&r, thh.header_hash.clone())?
                        .next()?
                        .is_none()
                {
                    return Ok(EntryDhtStatus::Live);
                }
            }

            Ok(EntryDhtStatus::Dead)
        })
    }

    async fn create_entry_details(&self, hash: EntryHash) -> CascadeResult<Option<EntryDetails>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        match self.get_entry_local_raw(&hash)? {
            Some(entry) => fresh_reader!(env, |r| {
                // Get the "headers that created this entry" hashes
                let headers = cache_data
                    .meta
                    .get_headers(&r, hash.clone())?
                    .chain(authored_data.meta.get_headers(&r, hash.clone())?)
                    .collect::<BTreeSet<_>>()?;

                // Get the rejected "headers that created this entry" hashes
                let rejected_headers = cache_data
                    .meta
                    .get_rejected_headers(&r, hash.clone())?
                    .chain(authored_data.meta.get_rejected_headers(&r, hash.clone())?)
                    .collect::<BTreeSet<_>>()?;

                // Get the delete hashes
                let deletes = cache_data
                    .meta
                    .get_deletes_on_entry(&r, hash.clone())?
                    .chain(authored_data.meta.get_deletes_on_entry(&r, hash.clone())?)
                    .collect::<BTreeSet<_>>()?;

                // Get the update hashes
                let updates = cache_data
                    .meta
                    .get_updates(&r, hash.clone().into())?
                    .chain(authored_data.meta.get_updates(&r, hash.into())?)
                    .collect::<BTreeSet<_>>()?;

                let entry_dht_status =
                    Self::compute_entry_dht_status(&headers, &cache_data, &authored_data, &env)?;

                // Render headers
                let headers = self.render_headers(headers, |h| {
                    h == HeaderType::Update || h == HeaderType::Create
                })?;
                let rejected_headers = self.render_headers(rejected_headers, |h| {
                    h == HeaderType::Update || h == HeaderType::Create
                })?;
                let deletes = self.render_headers(deletes, |h| h == HeaderType::Delete)?;
                let updates = self.render_headers(updates, |h| h == HeaderType::Update)?;
                Ok(Some(EntryDetails {
                    entry: entry.into_content(),
                    headers,
                    rejected_headers,
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
        let authored_data = ok_or_return!(self.authored_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        match self.get_element_local_raw(&hash)? {
            Some(element) => {
                let hash = element.header_address().clone();
                let (deletes, updates, validation_status) = fresh_reader!(env, |r| {
                    let deletes = cache_data
                        .meta
                        .get_deletes_on_header(&r, hash.clone())?
                        .chain(authored_data.meta.get_deletes_on_header(&r, hash.clone())?)
                        .collect::<BTreeSet<_>>()?;
                    let updates = cache_data
                        .meta
                        .get_updates(&r, hash.clone().into())?
                        .chain(authored_data.meta.get_updates(&r, hash.clone().into())?)
                        .collect::<BTreeSet<_>>()?;
                    let validation_status = cache_data.meta.get_validation_status(&r, &hash)?;
                    DatabaseResult::Ok((deletes, updates, validation_status))
                })?;
                let validation_status = validation_status
                    .resolve()
                    .unwrap_or(ValidationStatus::Valid);
                let deletes = self.render_headers(deletes, |h| h == HeaderType::Delete)?;
                let updates = self.render_headers(updates, |h| h == HeaderType::Update)?;
                Ok(Some(ElementDetails {
                    element,
                    validation_status,
                    deletes,
                    updates,
                }))
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
        Ok(integrated_data
            .meta
            .has_valid_registered_store_element(&hash)?
            || cache_data.meta.has_valid_registered_store_element(&hash)?
            || authored_data
                .meta
                .has_valid_registered_store_element(&hash)?)
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
        let get_call = options.strategy;
        let mut options: NetworkGetOptions = options.into();
        options.all_live_headers_with_metadata = true;
        let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;

        if authority {
            // Authorities only need to return local data
            // Short circuit as the authority
            self.update_cache_from_integrated(entry_hash.clone().into(), options)?;
        } else {
            // If the caller only needs the content we and we have the
            // content locally we can avoid the network call and return early.
            if let GetStrategy::Content = get_call {
                // Found local data return early.
                if let Some(result) = self.create_entry_details(entry_hash.clone()).await? {
                    return Ok(Some(result));
                }
            }
            // Update the cache from the network
            self.fetch_element_via_entry(entry_hash.clone(), options)
                .await?;
        }
        // Get the entry and metadata
        self.create_entry_details(entry_hash).await
    }

    /// Find the oldest live element in either the authored or cache stores
    fn get_oldest_live_element(&self, entry_hash: &EntryHash) -> CascadeResult<Search> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), Search::NotInCascade);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), Search::NotInCascade);
        let env = ok_or_return!(self.env.as_ref(), Search::NotInCascade);

        // Meta Cache and Meta Authored
        self.get_oldest_live_element_inner(
            &entry_hash,
            authored_data,
            &DbPair::from(cache_data),
            &env,
        )
    }

    fn get_oldest_live_element_inner<
        AnyPrefix: PrefixType,
        MA: MetadataBufT<AuthoredPrefix>,
        MC: MetadataBufT<AnyPrefix>,
    >(
        &self,
        entry_hash: &EntryHash,
        authored_data: &DbPair<MA, AuthoredPrefix>,
        cache_data: &DbPair<MC, AnyPrefix>,
        env: &DbRead,
    ) -> CascadeResult<Search> {
        fresh_reader!(env, |r| {
            let oldest_live_header = authored_data
                .meta
                .get_headers(&r, entry_hash.clone())?
                .chain(cache_data.meta.get_headers(&r, entry_hash.clone())?)
                .filter_map(|header| {
                    if authored_data
                        .meta
                        .get_deletes_on_header(&r, header.header_hash.clone())?
                        .next()?
                        .is_none()
                        && cache_data
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
                .min()?;

            match oldest_live_header {
                Some(oldest_live_header) => {
                    // Check we don't have evidence of an invalid header
                    if cache_data
                        .meta
                        .get_validation_status(&r, &oldest_live_header.header_hash)?
                        .is_valid()
                    {
                        // We have an oldest live header now get the element
                        Ok(self
                            .get_element_local_raw(&oldest_live_header.header_hash)?
                            .map(Search::Found)
                            // It's not local so check the network
                            .unwrap_or(Search::Continue(oldest_live_header.header_hash)))
                    } else {
                        Ok(Search::Continue(oldest_live_header.header_hash))
                    }
                }
                None => Ok(Search::NotInCascade),
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
        let get_call = options.strategy;
        let mut oldest_live_element = Search::NotInCascade;
        let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
        let authoring = self.am_i_authoring(&entry_hash.clone().into()).await?;

        // If this agent is in the process of authoring then
        // there is no reason to go to the network
        if authoring {
            oldest_live_element = self.get_oldest_live_element(&entry_hash)?;
        } else if authority {
            // Short circuit as the authority
            self.update_cache_from_integrated(entry_hash.clone().into(), options.clone().into())?;
            oldest_live_element = self.get_oldest_live_element(&entry_hash)?;
        } else {
            // If the caller only needs the content we and we have the
            // content locally we can avoid the network call
            if let GetStrategy::Content = get_call {
                oldest_live_element = self.get_oldest_live_element(&entry_hash)?;
            }
            // Was not found locally so go to the network
            if let Search::NotInCascade = oldest_live_element {
                // Update the cache from the network
                self.fetch_element_via_entry(entry_hash.clone(), options.clone().into())
                    .await?;

                oldest_live_element = self.get_oldest_live_element(&entry_hash)?;
            }
        }

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
        let get_call = options.strategy;
        let mut options: NetworkGetOptions = options.into();
        options.all_live_headers_with_metadata = true;

        let authority = self.am_i_an_authority(header_hash.clone().into()).await?;
        let authoring = self.am_i_authoring(&header_hash.clone().into()).await?;

        // If this agent is in the process of authoring then
        // there is no reason to go to the network
        if authoring {
        } else if authority {
            // Short circuit. This makes sense for full sharding.
            self.update_cache_from_integrated(header_hash.clone().into(), options)?;
        } else {
            // If the caller only needs the content we and we have the
            // content locally we can avoid the network call and return early.
            if let GetStrategy::Content = get_call {
                // Found local data return early.
                if let Some(result) = self.create_element_details(header_hash.clone())? {
                    return Ok(Some(result));
                }
            }
            // Network
            self.fetch_element_via_header(header_hash.clone(), options)
                .await?;
        }

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

        let get_call = options.strategy;
        let authority = self.am_i_an_authority(header_hash.clone().into()).await?;
        let authoring = self.am_i_authoring(&header_hash.clone().into()).await?;

        // If this agent is in the process of authoring then
        // there is no reason to go to the network
        if authoring {
        } else if authority {
            // Short circuit. This makes sense for full sharding.
            self.update_cache_from_integrated(header_hash.clone().into(), options.clone().into())?;
        } else {
            // If the caller only needs the content we and we have the
            // content locally we can avoid the network call and return early.
            if let GetStrategy::Content = get_call {
                // Found local data return early.
                if let Some(result) = self.dht_get_header_inner(header_hash.clone())? {
                    return Ok(Some(result));
                }
            }
            // Network
            self.fetch_element_via_header(header_hash.clone(), options.into())
                .await?;
        }

        self.dht_get_header_inner(header_hash)
    }

    fn dht_get_header_inner(&self, header_hash: HeaderHash) -> CascadeResult<Option<Element>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        fresh_reader!(env, |r| {
            // Check if header is alive after fetch
            let is_live = cache_data
                .meta
                .get_deletes_on_header(&r, header_hash.clone())?
                .next()?
                .is_none();

            // Check if the header is valid
            let is_live = is_live
                && cache_data
                    .meta
                    .get_validation_status(&r, &header_hash)?
                    .is_valid();

            if is_live {
                self.get_element_local_raw(&header_hash)
            } else {
                Ok(None)
            }
        })
    }

    /// Same as retrieve entry but retrieves many
    /// entries in parallel
    pub async fn retrieve_entries_parallel<'iter, I: IntoIterator<Item = EntryHash>>(
        &mut self,
        hashes: I,
        options: NetworkGetOptions,
    ) -> CascadeResult<Vec<Option<EntryHashed>>> {
        // Gather the entries we have locally on the left and
        // the entries we must fetch on the right.
        let mut entries = Vec::new();
        let mut to_fetch = Vec::new();
        for hash in hashes {
            match self.get_entry_local_raw(&hash)? {
                // This entry is local so nothing else to do.
                Some(e) => entries.push(Either::Left(Some(e))),
                // This entry needs to be fetched.
                // It is added to the to_fetch and the hash is also stored
                // in entries so we can preserve the order.
                None => {
                    entries.push(Either::Right(hash.clone()));
                    to_fetch.push(hash);
                }
            }
        }

        // Fetch all the entries in parallel
        self.fetch_elements_via_entry_parallel(to_fetch, options)
            .await?;

        // TODO: Could return this iterator rather then collecting but I couldn't solve the lifetimes.

        // Entries are returned as options because the caller might care if some were not found.
        fallible_iterator::convert(entries.into_iter().map(Ok))
            .map(|either| match either {
                // Entries on the left we have.
                Either::Left(option) => Ok(option),
                // Entries on the right we will try to get from the cache
                // again because there has been a fetch.
                Either::Right(hash) => Ok(self.get_entry_local_raw(&hash)?),
            })
            .collect()
    }

    /// Same as retrieve_header but retrieves many
    /// elements in parallel
    pub async fn retrieve_headers_parallel<'iter, I: IntoIterator<Item = HeaderHash>>(
        &mut self,
        hashes: I,
        options: NetworkGetOptions,
    ) -> CascadeResult<Vec<Option<SignedHeaderHashed>>> {
        // Gather the elements we have locally on the left and
        // the elements we must fetch on the right.
        let mut headers = Vec::new();
        let mut to_fetch = Vec::new();
        for hash in hashes {
            match self.get_header_local_raw_with_sig(&hash)? {
                // This element is local so nothing else to do.
                Some(e) => headers.push(Either::Left(Some(e))),
                // This entry needs to be fetched.
                // It is added to the to_fetch and the hash is also stored
                // in entries so we can preserve the order.
                None => {
                    headers.push(Either::Right(hash.clone()));
                    to_fetch.push(hash);
                }
            }
        }

        // Fetch all the entries in parallel
        self.fetch_elements_via_header_parallel(to_fetch, options)
            .await?;

        // TODO: Could return this iterator rather then collecting but I couldn't solve the lifetimes.

        // Entries are returned as options because the caller might care if some were not found.
        fallible_iterator::convert(headers.into_iter().map(Ok))
            .map(|either| match either {
                // Entries on the left we have.
                Either::Left(option) => Ok(option),
                // Entries on the right we will try to get from the cache
                // again because there has been a fetch.
                Either::Right(hash) => Ok(self.get_header_local_raw_with_sig(&hash)?),
            })
            .collect()
    }

    /// Same as retrieve but retrieves many
    /// elements in parallel
    pub async fn retrieve_parallel<'iter, I: IntoIterator<Item = HeaderHash>>(
        &mut self,
        hashes: I,
        options: NetworkGetOptions,
    ) -> CascadeResult<Vec<Option<Element>>> {
        // Gather the elements we have locally on the left and
        // the elements we must fetch on the right.
        let mut elements = Vec::new();
        let mut to_fetch = Vec::new();
        for hash in hashes {
            match self.get_element_local_raw(&hash)? {
                // This element is local so nothing else to do.
                Some(e) => elements.push(Either::Left(Some(e))),
                // This entry needs to be fetched.
                // It is added to the to_fetch and the hash is also stored
                // in entries so we can preserve the order.
                None => {
                    elements.push(Either::Right(hash.clone()));
                    to_fetch.push(hash);
                }
            }
        }

        // Fetch all the entries in parallel
        self.fetch_elements_via_header_parallel(to_fetch, options)
            .await?;

        // TODO: Could return this iterator rather then collecting but I couldn't solve the lifetimes.

        // Entries are returned as options because the caller might care if some were not found.
        fallible_iterator::convert(elements.into_iter().map(Ok))
            .map(|either| match either {
                // Entries on the left we have.
                Either::Left(option) => Ok(option),
                // Entries on the right we will try to get from the cache
                // again because there has been a fetch.
                Either::Right(hash) => Ok(self.get_element_local_raw(&hash)?),
            })
            .collect()
    }

    /// Get the entry from the dht regardless of metadata or validation status.
    /// This call has the opportunity to hit the local cache
    /// and avoid a network call.
    // TODO: This still fetches the full element and metadata.
    // Need to add a fetch_retrieve_entry that only gets data.
    pub async fn retrieve_entry(
        &mut self,
        hash: EntryHash,
        options: NetworkGetOptions,
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
        options: NetworkGetOptions,
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
        options: NetworkGetOptions,
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
        options: GetOptions,
    ) -> CascadeResult<Option<Details>> {
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
        if self.am_i_an_authority(key.base().clone().into()).await? {
            // Short circuit. This makes sense for full sharding.
            self.update_link_cache_from_integrated(key, options)?;
        } else {
            // Update the cache from the network
            self.fetch_links(key.into(), options).await?;
        }

        let cache_data = ok_or_return!(self.cache_data.as_ref(), vec![]);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), vec![]);
        let env = ok_or_return!(self.env.as_ref(), vec![]);
        fresh_reader!(env, |r| {
            // Meta Cache
            // Return any links from the meta cache that don't have removes.
            let mut links = cache_data
                .meta
                .get_live_links(&r, key)?
                .map(|l| Ok(l.into_link()))
                .chain(
                    authored_data
                        .meta
                        .get_live_links(&r, key)?
                        .map(|l| Ok(l.into_link())),
                )
                // Need to collect into a Set first to remove
                // duplicates from authored and cache
                .collect::<Vec<_>>()?;
            links.sort_by_key(|l| l.timestamp);
            links.dedup();
            Ok(links)
        })
    }

    #[instrument(skip(self, key, options))]
    /// Return all CreateLink headers
    /// and DeleteLink headers ordered by time.
    pub async fn get_link_details<'link>(
        &mut self,
        key: &'link LinkMetaKey<'link>,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)>> {
        if self.am_i_an_authority(key.base().clone().into()).await? {
            // Short circuit and update the cache from this cells authority data.
            self.update_link_cache_from_integrated(key, options)?;
        } else {
            // Update the cache from the network
            self.fetch_links(key.into(), options).await?;
        }

        let cache_data = ok_or_return!(self.cache_data.as_ref(), vec![]);
        let authored_data = ok_or_return!(self.authored_data.as_ref(), vec![]);
        let env = ok_or_return!(self.env.as_ref(), vec![]);
        // Get the links and collect the CreateLink / DeleteLink hashes by time.
        // Search authored and combine with cache_data
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
                    // Return all link removes with this link add
                    Ok((link_add.link_add_hash, link_removes))
                })
                .chain(authored_data.meta.get_links_all(&r, key)?.map(|link_add| {
                    // Collect the link removes on this link add
                    let link_removes = authored_data
                        .meta
                        .get_link_removes_on_link_add(&r, link_add.link_add_hash.clone())?
                        .collect::<BTreeSet<_>>()?;
                    // Return all link removes with this link add
                    Ok((link_add.link_add_hash, link_removes))
                }))
                .collect::<BTreeMap<_, _>>()
        })?;
        // Get the headers from the element stores
        fallible_iterator::convert(links.into_iter().map(Ok))
            .filter_map(|(create_link, delete_links)| {
                // Get the create link data
                match self.get_header_local_raw_with_sig(&create_link)? {
                    Some(create_link)
                        if create_link.header().header_type() == HeaderType::CreateLink =>
                    {
                        // Render the delete links making sure they are DeleteLink headers
                        let delete_links =
                            self.render_headers(delete_links, |h| h == HeaderType::DeleteLink)?;
                        Ok(Some((create_link, delete_links)))
                    }
                    // Not a create link
                    Some(_) => Ok(None),
                    // No header found
                    None => Ok(None),
                }
            })
            .collect()
    }

    async fn fetch_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let all_agent_activity = network.get_agent_activity(agent, query, options).await?;
        for agent_activity in all_agent_activity {
            self.update_agent_activity_stores(agent_activity).await?;
        }
        Ok(())
    }

    async fn fetch_agent_activity_status(
        &mut self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        mut options: GetActivityOptions,
    ) -> CascadeResult<()> {
        options.include_valid_activity = false;
        options.include_rejected_activity = false;
        options.include_full_headers = false;
        // TODO: Maybe this could short circuit in full sharding? But I'm not sure.
        // Skipping this for now.
        self.fetch_agent_activity(agent.clone(), query.clone(), options)
            .await?;
        Ok(())
    }

    fn get_agent_activity_from_cache(
        agent: AgentPubKey,
        range: &Option<std::ops::Range<u32>>,
        cache_data: &DbPairMut<'a, MetaCache>,
        env: &DbRead,
    ) -> CascadeResult<Vec<(u32, HeaderHash)>> {
        match range {
            Some(range) => {
                // One less than the end of an exclusive range is actually
                // the last header we want in the chain.
                fresh_reader!(env, |r| {
                    // Check if we have up to that header in the metadata store.
                    if cache_data
                        .meta
                        .get_activity(
                            &r,
                            ChainItemKey::AgentStatusSequence(
                                agent.clone(),
                                ValidationStatus::Valid,
                                range.end - 1,
                            ),
                        )?
                        .next()?
                        .is_some()
                    {
                        // We have the chain so collect the hashes in order of header sequence.
                        // Note if the chain is forked there could be multiple headers at each sequence number.
                        Ok(cache_data
                            .meta
                            .get_activity_sequence(
                                &r,
                                ChainItemKey::AgentStatus(agent, ValidationStatus::Valid),
                            )?
                            // TODO: PERF: Use an iter from to start from the correct sequence
                            .skip_while(|(s, _)| Ok(*s < range.start))
                            .take_while(|(s, _)| Ok(*s < range.end))
                            .collect()?)
                    } else {
                        // The requested chain is not in our cache.
                        Ok(vec![])
                    }
                })
            }
            // Requesting full chain so return all everything we have
            None => fresh_reader!(env, |r| {
                Ok(cache_data
                    .meta
                    .get_activity_sequence(
                        &r,
                        ChainItemKey::AgentStatus(agent, ValidationStatus::Valid),
                    )?
                    .collect()?)
            }),
        }
    }

    /// Check if we have a cache hit on a valid chain
    /// and return the hashes if we do.
    fn find_valid_activity_cache_hit(
        &self,
        agent: AgentPubKey,
        sequence_range: &Option<std::ops::Range<u32>>,
    ) -> CascadeResult<Option<Vec<(u32, HeaderHash)>>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);

        // Check if the range contains any values.
        // This also makes it safe to do `range.end - 1`
        match sequence_range {
            // The range is empty so there's not hashes to get
            Some(r) if r.end == 0 => return Ok(Some(vec![])),
            // It only makes sense to check the cache first if
            // a range has been requested otherwise
            // we must go to the network because we don't
            // know how long the chain is.
            None => return Ok(None),
            _ => {}
        }
        // Try getting the activity from the cache.
        let chain_hashes =
            Self::get_agent_activity_from_cache(agent.clone(), sequence_range, cache_data, env)?;

        // Get the current status
        let cached_status = cache_data.meta.get_activity_status(&agent)?;

        // If the chain is valid and the header we need is equal or below
        // and the hashes length is equal to one more then the last header sequence number
        // then we have a cache valid hit
        match (chain_hashes.last(), &cached_status) {
            (Some((chain_head_seq, _)), Some(ChainStatus::Valid(valid_status)))
                if *chain_head_seq <= valid_status.header_seq
                    && chain_hashes.len() as u32 == chain_head_seq + 1 =>
            {
                Ok(Some(chain_hashes))
            }
            _ => Ok(None),
        }
    }

    /// Do a full fetch of hashes and return the activity
    async fn fetch_and_create_activity(
        &mut self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse<Element>> {
        // Fetch the activity from the network
        // TODO: Maybe this could short circuit in full sharding? But I'm not sure.
        // Skipping this for now.
        self.fetch_agent_activity(agent.clone(), query.clone(), options)
            .await?;

        let cache_data = ok_or_return!(
            self.cache_data.as_ref(),
            AgentActivityResponse::empty(&agent)
        );
        let env = ok_or_return!(self.env.as_ref(), AgentActivityResponse::empty(&agent));
        // Now try getting the latest activity from cache
        let hashes = Self::get_agent_activity_from_cache(
            agent.clone(),
            &query.sequence_range,
            cache_data,
            env,
        )?;
        self.create_activity(agent, hashes)
    }

    /// Turn the hashes into agent activity with status and highest_observed
    // TODO: There are several parts missing to this function because we
    // are currently constraining the behavior to only serve getting validation
    // packages.
    // - [ ] Return the rejected activity (with or without caching it)
    // - [ ] Be able to handle full headers as well as hashes (with or without caching)
    // - [ ] Maybe Empty chains should not be set to NotRequested and set to the
    // value that reflects the requester
    fn create_activity(
        &self,
        agent: AgentPubKey,
        hashes: Vec<(u32, HeaderHash)>,
    ) -> CascadeResult<AgentActivityResponse<Element>> {
        let cache_data = ok_or_return!(
            self.cache_data.as_ref(),
            AgentActivityResponse::empty(&agent)
        );
        // Now try getting the latest activity from cache
        let highest_observed = cache_data.meta.get_activity_observed(&agent)?;
        match cache_data.meta.get_activity_status(&agent)? {
            Some(status) => Ok(AgentActivityResponse {
                agent,
                valid_activity: ChainItems::Hashes(hashes),
                rejected_activity: ChainItems::NotRequested,
                status,
                highest_observed,
            }),
            // If we don't have any status then we must return an empty chain
            None => Ok(AgentActivityResponse {
                agent,
                valid_activity: ChainItems::NotRequested,
                rejected_activity: ChainItems::NotRequested,
                status: ChainStatus::Empty,
                highest_observed,
            }),
        }
    }

    // TODO: The whole chain needs to be retrieved so we can
    // check if the headers match the filter but we could store
    // header types / entry types in the activity db to avoid this.
    #[instrument(skip(self, agent, query, options))]
    /// Get agent activity from agent activity authorities.
    /// Hashes are requested from the authority and cache for valid chains.
    /// Options:
    /// - include_valid_activity will include the valid chain hashes.
    /// - include_rejected_activity will include the valid chain hashes. (unimplemented)
    /// - include_full_headers will fetch the valid headers in parallel (requires include_valid_activity)
    /// Query:
    /// - include_entries will also fetch the entries in parallel (requires include_full_headers)
    /// - sequence_range will get all the activity in the exclusive range
    /// - header_type and entry_type will filter the activity (requires include_full_headers)
    pub async fn get_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        mut options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse<Element>> {
        const DEFAULT_ACTIVITY_TIMEOUT_MS: u64 = 1000;
        // Get the request options
        let requester_options = options.clone();
        // For now only fetching hashes until caching is worked out.
        options.include_full_headers = false;
        // Get agent activity takes longer then other calls so we
        // will give it a larger default timeout
        options.timeout_ms = options
            .timeout_ms
            .clone()
            .or(Some(DEFAULT_ACTIVITY_TIMEOUT_MS));

        // See if we have a cache hit
        let chain_hashes = match &query.sequence_range {
            Some(_) => {
                // If we have some cached agent activity then don't fetch the activity.
                // Instead fetch just the status and see if the chain is still valid
                // up to that point.
                //
                // If it's different then do a full fetch.
                // Fetch status without activity

                // Fetch just the status
                self.fetch_agent_activity_status(agent.clone(), query.clone(), options.clone())
                    .await?;

                // See if our cache is still valid
                self.find_valid_activity_cache_hit(agent.clone(), &query.sequence_range)?
            }
            None => None,
        };

        // Create the activity
        let mut activity = match chain_hashes {
            // If there was no activity in the cache then try fetching it
            None => {
                self.fetch_and_create_activity(agent.clone(), query.clone(), options.clone())
                    .await?
            }
            // Create the activity from the hashes
            Some(chain_hashes) => self.create_activity(agent.clone(), chain_hashes)?,
        };

        // Check if we are done
        match &activity {
            // ChainItems is empty so nothing else to do.
            AgentActivityResponse {
                status: ChainStatus::Empty,
                ..
            } => return Ok(activity),
            // ChainItems has a status but there are no hashes
            // so nothing else to do.
            AgentActivityResponse {
                valid_activity: ChainItems::Hashes(h),
                ..
            } if h.is_empty() => {
                if requester_options.include_full_headers {
                    activity.valid_activity = ChainItems::Full(Vec::new());
                }
                return Ok(activity);
            }
            _ => {}
        }

        match &activity.valid_activity {
            ChainItems::Full(_) => todo!(),
            ChainItems::Hashes(hashes) => {
                // If full headers and include entries is requested
                // retrieve them in parallel
                if query.include_entries && requester_options.include_full_headers {
                    let hashes = hashes.iter().map(|(_, h)| h.clone());
                    let mut elements = self
                        .retrieve_activity_elements(hashes.clone(), &query)
                        .await?;
                    let mut retry_gets = requester_options.retry_gets;
                    while elements.is_none() && retry_gets > 0 {
                        retry_gets -= 1;
                        elements = self
                            .retrieve_activity_elements(hashes.clone(), &query)
                            .await?;
                    }
                    let elements = elements.unwrap_or_else(Vec::new);
                    Ok(AgentActivityResponse {
                        valid_activity: ChainItems::Full(elements),
                        ..activity
                    })
                // If only full headers is requested
                // retrieve just the headers in parallel
                } else if requester_options.include_full_headers {
                    let hashes = hashes.iter().map(|(_, h)| h.clone());
                    let mut elements = self
                        .retrieve_activity_headers(hashes.clone(), &query)
                        .await?;
                    let mut retry_gets = requester_options.retry_gets;
                    while elements.is_none() && retry_gets > 0 {
                        retry_gets -= 1;
                        elements = self
                            .retrieve_activity_headers(hashes.clone(), &query)
                            .await?;
                    }
                    let elements = elements.unwrap_or_else(Vec::new);
                    Ok(AgentActivityResponse {
                        valid_activity: ChainItems::Full(elements),
                        ..activity
                    })
                } else {
                    // Otherwise return just the hashes
                    Ok(activity)
                }
            }
            ChainItems::NotRequested => Ok(activity),
        }
    }

    async fn retrieve_activity_elements(
        &mut self,
        hashes: impl IntoIterator<Item = HeaderHash>,
        query: &ChainQueryFilter,
    ) -> CascadeResult<Option<Vec<Element>>> {
        Ok(self
            .retrieve_parallel(hashes, Default::default())
            .await?
            .into_iter()
            // Filter the headers by the query
            .filter(|o| match o {
                Some(el) => query.check(el.header()),
                None => true,
            })
            .collect::<Option<Vec<_>>>())
    }

    async fn retrieve_activity_headers(
        &mut self,
        hashes: impl IntoIterator<Item = HeaderHash>,
        query: &ChainQueryFilter,
    ) -> CascadeResult<Option<Vec<Element>>> {
        Ok(self
            .retrieve_headers_parallel(hashes, Default::default())
            .await?
            .into_iter()
            // Filter the headers by the query
            .filter(|o| match o {
                Some(el) => query.check(el.header()),
                None => true,
            })
            .map(|shh| shh.map(|s| Element::new(s, None)))
            .collect::<Option<Vec<_>>>())
    }

    /// Get the validation package if it is cached without going to the network
    pub fn get_validation_package_local(
        &self,
        hash: &HeaderHash,
    ) -> CascadeResult<Option<Vec<Element>>> {
        let cache_data = ok_or_return!(self.cache_data.as_ref(), None);
        let env = ok_or_return!(self.env.as_ref(), None);
        fresh_reader!(env, |r| {
            let mut iter = cache_data.meta.get_validation_package(&r, hash)?;
            let mut elements = Vec::with_capacity(iter.size_hint().0);
            while let Some(hash) = iter.next()? {
                match self.get_element_local_raw(&hash)? {
                    Some(el) => elements.push(el),
                    None => return Ok(None),
                }
            }
            elements.sort_unstable_by_key(|el| el.header().header_seq());
            elements.reverse();
            if elements.is_empty() {
                Ok(None)
            } else {
                Ok(Some(elements))
            }
        })
    }

    pub async fn get_validation_package(
        &mut self,
        agent: AgentPubKey,
        header: &HeaderHashed,
    ) -> CascadeResult<Option<ValidationPackage>> {
        if let Some(elements) = self.get_validation_package_local(header.as_hash())? {
            return Ok(Some(ValidationPackage::new(elements)));
        }

        let network = ok_or_return!(self.network.as_mut(), None);
        match network
            .get_validation_package(agent, header.as_hash().clone())
            .await?
            .0
        {
            Some(validation_package) => {
                for element in &validation_package.0 {
                    // TODO: I don't think it's sound to do this
                    // because we would be adding potentially rejected
                    // headers into our cache.
                    // TODO: For now we are only returning validation packages
                    // of valid headers but when we add the ability to get and
                    // cache invalid data we need to update this as well.
                    // TODO: Assuming validation packages are also valid headers.
                    // We will need to prove this is the case in the future.
                    self.update_stores(ElementStatus::new(
                        element.clone(),
                        ValidationStatus::Valid,
                    ))?;
                }

                // Add metadata for custom package caching
                let cache_data = ok_or_return!(self.cache_data.as_mut(), None);
                cache_data.meta.register_validation_package(
                    header.as_hash(),
                    validation_package
                        .0
                        .iter()
                        .map(|el| el.header_address().clone()),
                );

                Ok(Some(validation_package))
            }
            None => Ok(None),
        }
    }

    async fn am_i_authoring(&mut self, hash: &AnyDhtHash) -> CascadeResult<bool> {
        let authored_data = ok_or_return!(self.authored_data.as_ref(), false);
        Ok(authored_data.element.contains_in_scratch(&hash)?)
    }

    async fn am_i_an_authority(&mut self, hash: AnyDhtHash) -> CascadeResult<bool> {
        // TODO: IMPORTANT: Implement this when we start sharding.
        // We are always the authority in full sync dhts

        let integrated_data = ok_or_return!(self.integrated_data.as_ref(), false);
        let rejected_data = ok_or_return!(self.rejected_data.as_ref(), false);
        match *hash.hash_type() {
            AnyDht::Entry => Ok(integrated_data
                .element
                .contains_entry(&hash.clone().into())?
                || rejected_data.element.contains_entry(&hash.clone().into())?),
            AnyDht::Header => Ok(integrated_data
                .element
                .contains_header(&hash.clone().into())?
                || rejected_data
                    .element
                    .contains_header(&hash.clone().into())?),
        }
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

pub fn integrate_single_metadata<C, P>(
    op: DhtOpLight,
    element_store: &ElementBuf<P>,
    meta_store: &mut C,
) -> CascadeResult<()>
where
    P: PrefixType,
    C: MetadataBufT<P>,
{
    match op {
        DhtOpLight::StoreElement(hash, _, _) => {
            let header = get_header(hash, element_store)?;
            meta_store.register_element_header(&header)?;
        }
        DhtOpLight::StoreEntry(hash, _, _) => {
            let new_entry_header = get_header(hash, element_store)?.try_into()?;
            if let NewEntryHeader::Update(update) = &new_entry_header {
                meta_store.register_update(update.clone())?;
            }
            // Reference to headers
            meta_store.register_header(new_entry_header)?;
        }
        DhtOpLight::RegisterAgentActivity(hash, _) => {
            let header = get_header(hash, element_store)?;
            // register agent activity on this agents pub key
            meta_store.register_activity(&header, ValidationStatus::Valid)?;
        }
        DhtOpLight::RegisterUpdatedContent(hash, _, _)
        | DhtOpLight::RegisterUpdatedElement(hash, _, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.register_update(header)?;
        }
        DhtOpLight::RegisterDeletedEntryHeader(hash, _)
        | DhtOpLight::RegisterDeletedBy(hash, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.register_delete(header)?
        }
        DhtOpLight::RegisterAddLink(hash, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.add_link(header)?;
        }
        DhtOpLight::RegisterRemoveLink(hash, _) => {
            let header = get_header(hash, element_store)?.try_into()?;
            meta_store.delete_link(header)?;
        }
    }
    Ok(())
}

pub fn get_header<P: PrefixType>(
    hash: HeaderHash,
    element_store: &ElementBuf<P>,
) -> CascadeResult<Header> {
    Ok(element_store
        .get_header(&hash)?
        .ok_or_else(|| AuthorityDataError::MissingData(format!("{}", hash)))?
        .into_header_and_signature()
        .0
        .into_content())
}

#[cfg(test)]
/// Helper function for easily setting up cascades during tests
pub fn test_dbs_and_mocks(
    env: DbRead,
) -> (
    ElementBuf,
    holochain_state::metadata::MockMetadataBuf,
    ElementBuf,
    holochain_state::metadata::MockMetadataBuf,
) {
    let cas = ElementBuf::vault(env.clone().into(), true).unwrap();
    let element_cache = ElementBuf::cache(env.clone().into()).unwrap();
    let metadata = holochain_state::metadata::MockMetadataBuf::new();
    let metadata_cache = holochain_state::metadata::MockMetadataBuf::new();
    (cas, metadata, element_cache, metadata_cache)
}
