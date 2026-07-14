//! The Cascade is a multi-tiered accessor for Holochain DHT data.
//!
//! It is named "the cascade" because it performs cascading lookups across multiple sources.
//! In general, the flow is:
//! - Search in local storage, including the [`DhtStore`] and [`SyncScratch`].
//! - If the data are not found locally, request the data from the network. Network responses are
//!   cached into the [`DhtStore`]
//!
//! Not all lookups follow this pattern, namely links are treated differently. Where a lookup
//! doesn't follow this pattern, it is documented in the method documentation.
//!
//! In order to support apps that want to be able to operate offline, a [`GetStrategy`] can be
//! provided. Making a [`GetStrategy::Local`] request will avoid the network entirely. Making a
//! [`GetStrategy::Network`] request will allow the cascade to go to the network, but not force it
//! to. That means that if the requested data is available locally, the cascade won't attempt to
//! fetch it again.
//!
//! ## Retrieve vs Get
//!
//! There are two words used in cascade functions: "get", and "retrieve".
//! They mean distinct things:
//!
//! - "get" ignores invalid data, and sometimes takes into account CRUD metadata
//!   before returning the data, so for instance, Deletes
//!   are allowed to annihilate Creates so that neither is returned. This is a more
//!   "refined" form of fetching data.
//! - "retrieve" only fetches the data if it exists, without regard to validation status.
//!   This is a more "raw" form of fetching data.
#![deny(missing_docs)]

use crate::error::CascadeError;
use crate::get_options_ext::GetOptionsExt;
use error::CascadeResult;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_p2p::actor::GetLinksRequestOptions;
use holochain_p2p::actor::{GetActivityOptions, NetworkRequestOptions};
use holochain_p2p::{DynHolochainP2pDna, HolochainP2pError};
use holochain_state::dht_store::DhtStore;
use holochain_state::host_fn_workspace::HostFnStores;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::prelude::*;
use holochain_state::query::link::GetLinksFilter;
use holochain_state::scratch::SyncScratch;
use holochain_zome_types::prelude::{FunctionName, ZomeName};
use metrics::{cascade_duration_metric, cascade_fetch_error_metric};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tracing::*;
use verify::{rejected_without_warrant, verify_activity_signatures, verify_rendered_ops_batch};

/// Get an item from an option
/// or return early from the function
macro_rules! some_or_return {
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

pub mod authority;
pub mod error;
pub mod get_options_ext;

mod agent_activity;
mod fetch;
mod metrics;
mod verify;

#[cfg(feature = "test_utils")]
mod mock;

/// Marks whether data came from a local store or another node on the network
#[derive(Debug, Clone)]
pub enum CascadeSource {
    /// Data came from a local store
    Local,
    /// Data came from another node on the network
    Network,
}

/// Options for configuring cascade lookups.
#[derive(Debug, Clone, Default)]
pub struct CascadeOptions {
    /// Configure how the cascade makes network requests.
    pub network_request_options: NetworkRequestOptions,

    /// Options for controlling where data may be retrieved from.
    pub get_options: GetOptions,
}

/// The cascade is a multi-tiered accessor for Holochain DHT data.
#[derive(Clone)]
pub struct CascadeImpl {
    /// The [`DhtStore`] to read data from, and write cached responses to.
    dht_store: DhtStore,
    /// A network handle, for requesting data from other peers.
    network: Option<DynHolochainP2pDna>,
    /// A scratch space, for content that is has been authored but not yet written to the agent's
    /// source chain.
    ///
    /// This will only be available when the cascade is used from a zome invocation that may be
    /// authoring new records.
    scratch: Option<SyncScratch>,
    /// Identify the agent making the request for data, to permit looking up private data for that
    /// agent.
    private_data: Option<Arc<AgentPubKey>>,
    /// Optional zome call origin for metrics attribution.
    zome_call_origin: Option<(ZomeName, FunctionName)>,
}

/// Times a cascade query and records `hc.cascade.duration` on drop, so every
/// return path of a query method (including `?` early returns) is covered.
///
/// Only queries with a `zome_call_origin` are recorded: the metric is
/// attributed by `zome`/`fn`, and the origin-less cascades built by validation
/// and the `must_get_*` host fns would otherwise emit unattributed samples.
struct CascadeDurationGuard {
    start: Instant,
    /// Cloned from the cascade's `zome_call_origin`. Owned (not a `&self`
    /// borrow) so the guard can be held across the query's `.await` points
    /// without constraining the future's `Send`-ness.
    zome_call_origin: Option<(ZomeName, FunctionName)>,
}

impl Drop for CascadeDurationGuard {
    fn drop(&mut self) {
        let Some((zome, fn_name)) = &self.zome_call_origin else {
            return;
        };
        let attrs = [
            opentelemetry::KeyValue::new("zome", zome.to_string()),
            opentelemetry::KeyValue::new("fn", fn_name.to_string()),
        ];
        cascade_duration_metric().record(self.start.elapsed().as_secs_f64(), &attrs);
    }
}

impl CascadeImpl {
    /// Set the zome call origin for metrics attribution.
    pub fn with_zome_call_origin(self, zome_name: &ZomeName, fn_name: &FunctionName) -> Self {
        Self {
            zome_call_origin: Some((zome_name.clone(), fn_name.clone())),
            ..self
        }
    }

    /// Add the ability to access private entries for this agent.
    pub fn with_private_data(self, author: Arc<AgentPubKey>) -> Self {
        Self {
            private_data: Some(author),
            ..self
        }
    }

    /// Add the scratch to the cascade.
    pub fn with_scratch(self, scratch: SyncScratch) -> Self {
        Self {
            scratch: Some(scratch),
            ..self
        }
    }

    /// Add the network to the cascade.
    pub fn with_network(self, network: DynHolochainP2pDna) -> CascadeImpl {
        CascadeImpl {
            scratch: self.scratch,
            private_data: self.private_data,
            network: Some(network),
            dht_store: self.dht_store,

            zome_call_origin: self.zome_call_origin,
        }
    }

    /// Constructs a [Cascade] backed by the given [DhtStore].
    pub fn empty(dht_store: DhtStore) -> Self {
        Self {
            network: None,
            scratch: None,
            private_data: None,
            dht_store,

            zome_call_origin: None,
        }
    }

    /// Construct a [Cascade] with network access
    ///
    /// Accepts a writable or read-only workspace; the cascade only reads from
    /// the store it takes.
    pub fn from_workspace_and_network<Db>(
        workspace: &HostFnWorkspace<Db>,
        network: DynHolochainP2pDna,
    ) -> CascadeImpl
    where
        Db: AsRef<holochain_state::data::DbRead<holochain_state::data::Dht>>,
    {
        let HostFnStores { scratch, dht_store } = workspace.stores();
        let dht_store =
            dht_store.expect("HostFnWorkspace always populates dht_store; this is a bug");
        let private_data = workspace.author();
        CascadeImpl {
            private_data,
            scratch,
            network: Some(network),
            dht_store,
            zome_call_origin: None,
        }
    }

    /// Construct a [Cascade] with local-only access to the provided stores
    pub fn from_workspace_stores(stores: HostFnStores, author: Option<Arc<AgentPubKey>>) -> Self {
        let HostFnStores { scratch, dht_store } = stores;
        let dht_store =
            dht_store.expect("HostFnWorkspace always populates dht_store; this is a bug");
        Self {
            scratch,
            network: None,
            private_data: author,
            dht_store,
            zome_call_origin: None,
        }
    }

    /// Get an [`EntryDetails`], by its [`EntryHash`], which contains entry data along with all CRUD
    /// actions associated with it.
    ///
    /// Also returns rejected actions, which may affect the interpreted validity status of this
    /// [`Entry`].
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn get_entry_details(
        &self,
        entry_hash: EntryHash,
        options: CascadeOptions,
    ) -> CascadeResult<Option<EntryDetails>> {
        let _guard = self.time_cascade();
        let author = self.private_data.as_ref().map(|a| a.as_ref());
        let scratch = self.local_scratch();
        let read = self.dht_store.as_read();

        if options.get_options.strategy() == GetStrategy::Network {
            let authoring = self.am_i_authoring(&entry_hash.clone().into())?;
            let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
            if !(authoring || authority) {
                match self
                    .fetch_record(entry_hash.clone().into(), options.network_request_options)
                    .await
                {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch record from");
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(read
            .get_entry_details_with_scratch(&entry_hash, author, &scratch)
            .await?)
    }

    /// Get a [`RecordDetails`], by its [`ActionHash`], which contains a [`Record`] along with all
    /// updates and deletes associated with it.
    ///
    /// Can return a rejected record.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn get_record_details(
        &self,
        action_hash: ActionHash,
        options: CascadeOptions,
    ) -> CascadeResult<Option<RecordDetails>> {
        let _guard = self.time_cascade();
        let author = self.private_data.as_ref().map(|a| a.as_ref());
        let scratch = self.local_scratch();
        let read = self.dht_store.as_read();

        if options.get_options.strategy() == GetStrategy::Network {
            let authoring = self.am_i_authoring(&action_hash.clone().into())?;
            let authority = self.am_i_an_authority(action_hash.clone().into()).await?;
            if !(authoring || authority) {
                match self
                    .fetch_record(action_hash.clone().into(), options.network_request_options)
                    .await
                {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch record from");
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        Ok(read
            .get_record_details_with_scratch(&action_hash, author, &scratch)
            .await?)
    }

    /// Return a `SyncScratch` for use in DhtStore overlay reads.
    ///
    /// When the cascade has a scratch attached, that scratch is returned.
    /// Otherwise an empty scratch is returned so that the `*_with_scratch`
    /// methods on `DhtStoreRead` can be called unconditionally.
    fn local_scratch(&self) -> SyncScratch {
        self.scratch
            .clone()
            .unwrap_or_else(|| Scratch::new().into_sync())
    }

    /// Returns the [Record] for this [ActionHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    ///
    /// _Note: Deleted actions are a tombstone set_
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn dht_get_action(
        &self,
        action_hash: ActionHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        let _guard = self.time_cascade();
        // DESIGN: we can short circuit if we have any local deletes on an action.
        // Is this bad because we will not go back to the network until our
        // cache is cleared. Could someone create an attack based on this fact?
        let author = self.private_data.as_ref().map(|a| a.as_ref());
        let scratch = self.local_scratch();
        let read = self.dht_store.as_read();

        // Local read via DhtStore overlay.
        if let Some(record) = read
            .get_live_record_with_scratch(&action_hash, author, &scratch)
            .await?
        {
            return Ok(Some(record));
        }

        if options.strategy() == GetStrategy::Network {
            let authoring = self.am_i_authoring(&action_hash.clone().into())?;
            let authority = self.am_i_an_authority(action_hash.clone().into()).await?;
            if !(authoring || authority) {
                match self
                    .fetch_record(action_hash.clone().into(), options.to_network_options())
                    .await
                {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch record from");
                    }
                    Err(e) => return Err(e),
                }
            }
            // Re-read after network fetch.
            return Ok(read
                .get_live_record_with_scratch(&action_hash, author, &scratch)
                .await?);
        }

        Ok(None)
    }

    /// Returns the oldest live [Record] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn dht_get_entry(
        &self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        let _guard = self.time_cascade();
        let author = self.private_data.as_ref().map(|a| a.as_ref());
        let scratch = self.local_scratch();
        let read = self.dht_store.as_read();

        // Local read via DhtStore overlay.
        if let Some(record) = read
            .get_live_entry_with_scratch(&entry_hash, author, &scratch)
            .await?
        {
            return Ok(Some(record));
        }

        if options.strategy() == GetStrategy::Network {
            let authoring = self.am_i_authoring(&entry_hash.clone().into())?;
            let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
            if !(authoring || authority) {
                match self
                    .fetch_record(entry_hash.clone().into(), options.to_network_options())
                    .await
                {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch record from");
                    }
                    Err(e) => return Err(e),
                }
            }
            // Re-read after network fetch.
            return Ok(read
                .get_live_entry_with_scratch(&entry_hash, author, &scratch)
                .await?);
        }

        Ok(None)
    }

    /// Perform a concurrent `get` on multiple hashes simultaneously, returning
    /// the resulting list of Records in the order that they come in
    /// (NOT the order in which they were requested!).
    pub async fn get_concurrent<I: IntoIterator<Item = AnyDhtHash>>(
        &self,
        hashes: I,
        options: GetOptions,
    ) -> CascadeResult<Vec<Option<Record>>> {
        use futures::stream::StreamExt;
        use futures::stream::TryStreamExt;
        let iter = hashes.into_iter().map({
            |hash| {
                let options = options.clone();
                let cascade = self.clone();
                async move { cascade.dht_get(hash, options).await }
            }
        });
        futures::stream::iter(iter)
            .buffer_unordered(10)
            .try_collect()
            .await
    }

    /// Updates the cache with the latest network authority data
    /// and returns what is in the cache.
    /// This gives you the latest possible picture of the current dht state.
    /// Data from your zome call is also added to the cache.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub async fn dht_get(
        &self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        match hash.into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => self.dht_get_entry(hash, options).await,
            AnyDhtHashPrimitive::Action(hash) => self.dht_get_action(hash, options).await,
        }
    }

    /// Get either [`EntryDetails`] or [`RecordDetails`], depending on the hash provided
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    pub async fn get_details(
        &self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Details>> {
        match hash.into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => Ok(self
                .get_entry_details(
                    hash,
                    CascadeOptions {
                        network_request_options: options.to_network_options(),
                        get_options: options,
                    },
                )
                .await?
                .map(Details::Entry)),
            AnyDhtHashPrimitive::Action(hash) => Ok(self
                .get_record_details(
                    hash,
                    CascadeOptions {
                        network_request_options: options.to_network_options(),
                        get_options: options,
                    },
                )
                .await?
                .map(Details::Record)),
        }
    }

    /// Gets links from the DHT or cache depending on its metadata.
    /// Deleted or replaced entries are skipped.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn dht_get_links(
        &self,
        key: WireLinkKey,
        options: GetLinksRequestOptions,
    ) -> CascadeResult<Vec<Link>> {
        let _guard = self.time_cascade();
        // only fetch links from the network if I am not an authority and
        // GetStrategy is Network
        if let GetStrategy::Network = options.get_options.strategy() {
            let authority = self.am_i_an_authority(key.base.clone()).await?;
            if !authority {
                match self.fetch_links(key.clone(), options).await {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch links from");
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }

        let filter = GetLinksFilter {
            after: key.after,
            before: key.before,
            author: key.author,
        };

        let scratch = self.local_scratch();
        Ok(self
            .dht_store
            .as_read()
            .get_links_with_scratch(
                &key.base,
                &key.type_query,
                key.tag.as_ref(),
                &filter,
                &scratch,
            )
            .await?)
    }

    /// Return all CreateLink actions and DeleteLink actions ordered by time.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, key, options)))]
    pub async fn get_links_details(
        &self,
        key: WireLinkKey,
        options: GetLinksRequestOptions,
    ) -> CascadeResult<Vec<(SignedActionHashed, Vec<SignedActionHashed>)>> {
        let _guard = self.time_cascade();
        // only fetch link details from network if i am not an authority and
        // GetStrategy is Network
        if let GetStrategy::Network = options.get_options.strategy() {
            let authority = self.am_i_an_authority(key.base.clone()).await?;
            if !authority {
                match self.fetch_links(key.clone(), options).await {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch link details from");
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }
        let scratch = self.local_scratch();
        Ok(self
            .dht_store
            .as_read()
            .get_link_details_with_scratch(&key.base, &key.type_query, key.tag.as_ref(), &scratch)
            .await?)
    }

    /// Count the number of links matching the `query`.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, query)))]
    pub async fn dht_count_links(&self, query: WireLinkQuery) -> CascadeResult<usize> {
        let _guard = self.time_cascade();
        let mut links = HashSet::<ActionHash>::new();
        if !self.am_i_an_authority(query.base.clone()).await? {
            if let Some(network) = &self.network {
                match network
                    .count_links(
                        query.clone(),
                        NetworkRequestOptions::default(),
                        self.zome_call_origin.clone(),
                    )
                    .await
                {
                    Ok(actions) => {
                        links.extend(actions.create_link_actions());
                    }
                    Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                        // No peers available for this location, can't add new links to the cache
                        // at the moment.
                        tracing::debug!(?e, "No peers to fetch link count from");
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }
        }

        let filter = GetLinksFilter::from(query.clone());

        let scratch = self.local_scratch();
        links.extend(
            self.dht_store
                .as_read()
                .get_links_with_scratch(
                    &query.base,
                    &query.link_type,
                    query.tag_prefix.as_ref(),
                    &filter,
                    &scratch,
                )
                .await?
                .into_iter()
                .map(|l| l.create_link_hash),
        );

        Ok(links.len())
    }

    /// Request the chain of agent activity for an author, bounded by a given [`ChainFilter`]
    pub async fn must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: ChainFilter,
        options: NetworkRequestOptions,
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        let _guard = self.time_cascade();
        // Validate ChainFilter take is not zero.
        if filter.get_take() == Some(0) {
            return Err(CascadeError::InvalidInput(
                "ChainFilter take must be greater than 0".to_string(),
            ));
        }

        // DhtStore path: local read with scratch overlay.
        let local_scratch = self.local_scratch();
        let local_result = self
            .dht_store
            .as_read()
            .must_get_agent_activity_with_scratch(&author, &filter, &local_scratch)
            .await?;

        // If complete, return immediately.
        if matches!(local_result, MustGetAgentActivityResponse::Activity { .. }) {
            return Ok(local_result);
        }

        // If no network or we are an authority, return the local (incomplete) result.
        if self.network.is_none() || self.am_i_an_authority(author.clone().into()).await? {
            return Ok(local_result);
        }

        // Not complete and not an authority: try the network.
        // `fetch_must_get_agent_activity` writes the network response into the
        // DhtStore cache via `add_activity_into_cache`; we then re-read so the
        // freshly cached data is merged into the result.
        match self
            .fetch_must_get_agent_activity(author.clone(), filter.clone(), options)
            .await
        {
            Ok(_) => Ok(self
                .dht_store
                .as_read()
                .must_get_agent_activity_with_scratch(&author, &filter, &local_scratch)
                .await?),
            Err(CascadeError::NetworkError(e @ HolochainP2pError::NoPeersForLocation(_, _))) => {
                tracing::debug!(?e, "No peers to fetch must_get_agent_activity from");
                Ok(local_result)
            }
            Err(e) => Err(e),
        }
    }

    /// Get agent activity from agent activity authorities.
    ///
    /// Hashes are requested from the authority and cache for valid chains.
    ///
    /// Query:
    /// - [include_entries](ChainQueryFilter::include_entries) will also fetch the entries in parallel (requires include_full_records)
    /// - [sequence_range](ChainQueryFilter::sequence_range) will get all the activity in the exclusive range
    /// - [action_type](ChainQueryFilter::action_type) and [entry_type](ChainQueryFilter::entry_type) will filter the activity (requires include_full_actions)
    ///
    /// Options:
    /// - [include_valid_activity](GetActivityOptions::include_valid_activity) will include the valid chain hashes.
    /// - [include_rejected_activity](GetActivityOptions::include_rejected_activity) will include the invalid chain hashes.
    /// - [include_warrants](GetActivityOptions::include_warrants) will include the warrants for this agent.
    /// - [include_full_records](GetActivityOptions::include_full_records) will fetch the full records for each action matching the query.
    ///   This is only effective if [include_valid_activity](GetActivityOptions::include_valid_activity) or [include_rejected_activity](GetActivityOptions::include_rejected_activity) is true.
    ///   Even when this is set, entries will only be fetched if [include_entries](ChainQueryFilter::include_entries) is also true.
    #[cfg_attr(
        feature = "instrument",
        tracing::instrument(skip(self, agent, query, options))
    )]
    pub async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse> {
        let _guard = self.time_cascade();
        let status_only = !(options.include_valid_activity || options.include_rejected_activity);

        // If we're an authority then we allow local queries. This means we consider ourselves an authority
        // for the agent in question. If the options specify network, for example because we are looking for
        // warrants we don't know about or for countersigning actions, then we will go to the network
        // regardless of authority status.
        let authority = self.am_i_an_authority(agent.clone().into()).await?;

        let merged_response = if options.get_options.strategy() == GetStrategy::Local {
            // Local read via the DhtStore scratch overlay (no network needed).
            // This path is taken whether or not we're an authority for the
            // agent, so a self-read during a zome call returns the same result
            // either way — including authored-but-uncommitted activity held in
            // the scratch, which the authority serving path does not overlay.
            let dht_options = holochain_state::dht_store::GetAgentActivityOptions {
                include_valid_activity: options.include_valid_activity,
                include_rejected_activity: options.include_rejected_activity,
                include_warrants: options.include_warrants,
                include_full_records: options.include_full_records,
            };
            let scratch = self.local_scratch();
            self.dht_store
                .as_read()
                .get_agent_activity_with_scratch(&agent, &query, &dht_options, &scratch)
                .await?
        } else {
            // Network path: fetch from peers and merge.
            let results = self
                .fetch_agent_activity(agent.clone(), query.clone(), options.clone())
                .await?;
            let merged_response: AgentActivityResponse =
                agent_activity::merge_activities(agent.clone(), &options, results)?;

            // If there is a scratch and warrants were returned, add them to the scratch.
            // Only warrants coming from the network should be added to the scratch. Locally
            // found warrants shouldn't be redundantly added to the database.
            if !authority && !merged_response.warrants.is_empty() {
                if let Some(scratch) = &self.scratch {
                    if let Err(err) = scratch.apply(|scratch| {
                        for warrant in merged_response.warrants.iter() {
                            scratch.add_warrant(warrant.clone());
                        }
                    }) {
                        tracing::warn!(
                            ?err,
                            "Failed to add warrants from network response to scratch"
                        );
                    };
                }
            }

            merged_response
        };

        // If the response is empty we can finish.
        if let ChainStatus::Empty = &merged_response.status {
            return Ok(AgentActivityResponse::from_empty(merged_response));
        }

        // If the request is just for the status then return.
        if status_only {
            return Ok(AgentActivityResponse::status_only(merged_response));
        }

        let AgentActivityResponse {
            agent,
            mut valid_activity,
            mut rejected_activity,
            status,
            highest_observed,
            warrants,
        } = merged_response;

        // If records were requested then the activity authority might not have had all the entries.
        // That becomes more likely for new records as the number of agents on a network increases.
        // So we need to fill in the missing entries.
        if options.include_full_records && query.include_entries {
            tracing::debug!("Trying to fill missing entries for agent activity");
            valid_activity = self
                .fill_missing_chain_item_entries(valid_activity, options.get_options.clone())
                .await?;
            rejected_activity = self
                .fill_missing_chain_item_entries(rejected_activity, options.get_options)
                .await?;
        }

        let r = AgentActivityResponse {
            agent,
            valid_activity,
            rejected_activity,
            status,
            highest_observed,
            warrants,
        };

        Ok(r)
    }

    /// Looks through a [ChainItems] object and fills in any missing entry data.
    ///
    /// For any [RecordEntry::NotStored] entries, this function will attempt to fetch the entry data
    /// from either our cache when [GetOptions::local] is specified, or from the network when
    /// [GetOptions::network] is specified.
    ///
    /// Note that this will only take any action for [ChainItems::Full]. For other
    /// [ChainItems] variants, the function will just return its input.
    async fn fill_missing_chain_item_entries(
        &self,
        mut chain_items: ChainItems,
        get_options: GetOptions,
    ) -> CascadeResult<ChainItems> {
        let missing_entry_hashes = match &chain_items {
            ChainItems::Full(records) => records
                .iter()
                .filter_map(|r| match r.entry {
                    RecordEntry::NotStored => {
                        r.action().data.entry_hash().map(|h| h.clone().into())
                    }
                    _ => None,
                })
                .collect(),
            _ => Vec::with_capacity(0),
        };

        if !missing_entry_hashes.is_empty() {
            trace!(
                "There are {} missing entries to fetch",
                missing_entry_hashes.len()
            );

            let maybe_provided_entry_records = self
                .get_concurrent(missing_entry_hashes, get_options)
                .await?;

            trace!("Got {:?} entries", maybe_provided_entry_records.len());

            let entry_lookup = maybe_provided_entry_records
                .iter()
                .filter_map(|r| match r {
                    Some(r) => r
                        .action()
                        .data
                        .entry_hash()
                        .map(|entry_hash| (entry_hash, &r.entry)),
                    None => None,
                })
                .collect::<HashMap<_, _>>();

            match &mut chain_items {
                ChainItems::Full(records) => {
                    for record in records.iter_mut() {
                        if let RecordEntry::NotStored = record.entry {
                            if let Some(entry_hash) = record.action().data.entry_hash() {
                                if let Some(entry) = entry_lookup.get(entry_hash) {
                                    record.entry = (*entry).clone();
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Because of the match above, the valid activity should always be FullRecords
                    unreachable!()
                }
            }
        }

        Ok(chain_items)
    }

    fn am_i_authoring(&self, hash: &AnyDhtHash) -> CascadeResult<bool> {
        let scratch = some_or_return!(self.scratch.as_ref(), false);
        Ok(scratch.apply_and_then(|scratch| scratch.contains_hash(hash))?)
    }

    async fn am_i_an_authority(&self, hash: OpBasis) -> CascadeResult<bool> {
        let network = some_or_return!(self.network.as_ref(), false);
        Ok(network.authority_for_hash(hash).await?)
    }
}

/// A trait for accessing the cascade which can be mocked.
#[async_trait::async_trait]
#[cfg_attr(feature = "test_utils", mockall::automock)]
pub trait Cascade {
    /// Retrieve [`Entry`] either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    async fn retrieve_entry(
        &self,
        hash: EntryHash,
        mut options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(EntryHashed, CascadeSource)>>;

    /// Retrieve [`SignedActionHashed`] either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    async fn retrieve_action(
        &self,
        hash: ActionHash,
        mut options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(SignedActionHashed, CascadeSource)>>;

    /// Retrieve a complete [`Record`] either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    ///
    /// If the [`Action`] has an associated [`Entry`] and the entry is not
    /// available, `None` is returned. This applies to private entries too.
    //
    // This function is essential for fetching a warranted record, in cases where the action is
    // already present locally, but the entry is not. Returning the locally available
    // record without the entry would prevent a network request.
    async fn retrieve_public_record(
        &self,
        hash: AnyDhtHash,
        mut options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(Record, CascadeSource)>>;
}

#[async_trait::async_trait]
impl Cascade for CascadeImpl {
    async fn retrieve_entry(
        &self,
        hash: EntryHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(EntryHashed, CascadeSource)>> {
        let author = self.private_data.as_ref().map(|a| a.as_ref());
        let scratch = self.local_scratch();
        let read = self.dht_store.as_read();

        if let Some(entry) = read
            .retrieve_entry_with_scratch(&hash, author, &scratch)
            .await?
        {
            return Ok(Some((
                EntryHashed::from_content_sync(entry),
                CascadeSource::Local,
            )));
        }
        self.fetch_record(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let result = read
            .retrieve_entry_with_scratch(&hash, author, &scratch)
            .await?;
        Ok(result.map(|e| (EntryHashed::from_content_sync(e), CascadeSource::Network)))
    }

    async fn retrieve_action(
        &self,
        hash: ActionHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(SignedActionHashed, CascadeSource)>> {
        let scratch = self.local_scratch();
        let read = self.dht_store.as_read();

        if let Some(sah) = read.retrieve_action_with_scratch(&hash, &scratch).await? {
            return Ok(Some((sah, CascadeSource::Local)));
        }
        self.fetch_record(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let result = read.retrieve_action_with_scratch(&hash, &scratch).await?;
        Ok(result.map(|a| (a, CascadeSource::Network)))
    }

    async fn retrieve_public_record(
        &self,
        hash: AnyDhtHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(Record, CascadeSource)>> {
        // The DhtStore retrieve_record_with_scratch takes &ActionHash; dispatch on
        // hash type.  In practice all callers pass an ActionHash, but the trait
        // signature accepts AnyDhtHash so we must handle both.
        if let holo_hash::AnyDhtHashPrimitive::Action(action_hash) = hash.clone().into_primitive() {
            let author = self.private_data.as_ref().map(|a| a.as_ref());
            let scratch = self.local_scratch();
            let read = self.dht_store.as_read();

            if let Some(record) = read
                .retrieve_record_with_scratch(&action_hash, author, &scratch)
                .await?
            {
                return Ok(Some((record, CascadeSource::Local)));
            }
            self.fetch_record(hash.clone(), options).await?;

            // Check if we have the data now after the network call.
            let result = read
                .retrieve_record_with_scratch(&action_hash, author, &scratch)
                .await?;
            return Ok(result.map(|r| (r, CascadeSource::Network)));
        }

        // EntryHash variant: no DhtStore path available, fetch from network.
        self.fetch_record(hash.clone(), options).await?;
        Ok(None)
    }
}

/// Tests that wiring `CascadeImpl` onto a `DhtStore` + scratch correctly exposes
/// scratch-only content through the requester-read methods.
#[cfg(all(test, feature = "test_utils"))]
mod dht_store_scratch_overlay_tests;
