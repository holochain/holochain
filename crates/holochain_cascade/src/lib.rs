//! The Cascade is a multi-tiered accessor for Holochain DHT data.
//!
//! Note that the docs for this crate are admittedly a bit *loose and imprecise*,
//! but they are not expected to be *incorrect*.
//!
//! It is named "the Cascade" because it performs "cascading" gets across multiple sources.
//! In general (but not in all cases), the flow is something like:
//! - First attempts to read the local storage
//! - If that fails, attempt to read data from the network cache
//! - If that fails, do a network request for the data, caching it if found
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
//!
#![warn(missing_docs)]

use crate::error::CascadeError;
use error::CascadeResult;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_p2p::actor::GetLinksRequestOptions;
use holochain_p2p::actor::{GetActivityOptions, NetworkRequestOptions};
use holochain_p2p::{DynHolochainP2pDna, HolochainP2pError};
use holochain_state::host_fn_workspace::HostFnStores;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::mutations::insert_action;
use holochain_state::mutations::insert_entry;
use holochain_state::mutations::insert_op_lite;
use holochain_state::mutations::set_validation_status;
use holochain_state::prelude::*;
use holochain_state::query::entry_details::GetEntryDetailsQuery;
use holochain_state::query::link::{GetLinksFilter, GetLinksQuery};
use holochain_state::query::link_details::GetLinkDetailsQuery;
use holochain_state::query::live_entry::GetLiveEntryQuery;
use holochain_state::query::live_record::GetLiveRecordQuery;
use holochain_state::query::record_details::GetRecordDetailsQuery;
use holochain_state::query::DbScratch;
use holochain_state::query::PrivateDataQuery;
use holochain_state::scratch::SyncScratch;
use metrics::create_cascade_duration_metric;
use metrics::CascadeDurationMetric;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tracing::*;

pub mod authority;
pub mod error;

mod agent_activity;
mod metrics;

#[cfg(feature = "test_utils")]
pub mod test_utils;

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

/// The Cascade is a multi-tiered accessor for Holochain DHT data.
///
/// See the module-level docs for more info.
#[derive(Clone)]
pub struct CascadeImpl {
    authored: Option<DbRead<DbKindAuthored>>,
    dht: Option<DbRead<DbKindDht>>,
    cache: Option<DbWrite<DbKindCache>>,
    scratch: Option<SyncScratch>,
    network: Option<DynHolochainP2pDna>,
    private_data: Option<Arc<AgentPubKey>>,
    duration_metric: &'static CascadeDurationMetric,
}

impl CascadeImpl {
    /// Add the authored env to the cascade.
    pub fn with_authored(self, authored: DbRead<DbKindAuthored>) -> Self {
        Self {
            authored: Some(authored),
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

    /// Add the dht env to the cascade.
    pub fn with_dht(self, dht: DbRead<DbKindDht>) -> Self {
        Self {
            dht: Some(dht),
            ..self
        }
    }

    /// Add the cache to the cascade.
    pub fn with_cache(self, cache: DbWrite<DbKindCache>) -> Self {
        Self {
            cache: Some(cache),
            ..self
        }
    }

    /// Add the cache to the cascade.
    pub fn with_scratch(self, scratch: SyncScratch) -> Self {
        Self {
            scratch: Some(scratch),
            ..self
        }
    }

    /// Add the network and cache to the cascade.
    pub fn with_network(
        self,
        network: DynHolochainP2pDna,
        cache_db: DbWrite<DbKindCache>,
    ) -> CascadeImpl {
        CascadeImpl {
            authored: self.authored,
            dht: self.dht,
            scratch: self.scratch,
            private_data: self.private_data,
            cache: Some(cache_db),
            network: Some(network),
            duration_metric: create_cascade_duration_metric(),
        }
    }

    /// Constructs an empty [Cascade].
    pub fn empty() -> Self {
        Self {
            authored: None,
            dht: None,
            network: None,
            cache: None,
            scratch: None,
            private_data: None,
            duration_metric: create_cascade_duration_metric(),
        }
    }

    /// Construct a [Cascade] with network access
    pub fn from_workspace_and_network<AuthorDb, DhtDb>(
        workspace: &HostFnWorkspace<AuthorDb, DhtDb>,
        network: DynHolochainP2pDna,
    ) -> CascadeImpl
    where
        AuthorDb: ReadAccess<DbKindAuthored>,
        DhtDb: ReadAccess<DbKindDht>,
    {
        let HostFnStores {
            authored,
            dht,
            cache,
            scratch,
        } = workspace.stores();
        let private_data = workspace.author();
        CascadeImpl {
            authored: Some(authored),
            dht: Some(dht),
            cache: Some(cache),
            private_data,
            scratch,
            network: Some(network),
            duration_metric: create_cascade_duration_metric(),
        }
    }

    /// Construct a [Cascade] with local-only access to the provided stores
    pub fn from_workspace_stores(stores: HostFnStores, author: Option<Arc<AgentPubKey>>) -> Self {
        let HostFnStores {
            authored,
            dht,
            cache,
            scratch,
        } = stores;
        Self {
            authored: Some(authored),
            dht: Some(dht),
            cache: Some(cache),
            scratch,
            network: None,
            private_data: author,
            duration_metric: create_cascade_duration_metric(),
        }
    }

    /// Getter
    pub fn cache(&self) -> Option<&DbWrite<DbKindCache>> {
        self.cache.as_ref()
    }

    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_rendered_op(txn: &mut Txn<DbKindCache>, op: &RenderedOp) -> CascadeResult<()> {
        let RenderedOp {
            op_light,
            op_hash,
            action,
            validation_status,
        } = op;
        let op_order = OpOrder::new(op_light.get_type(), action.action().timestamp());
        let timestamp = action.action().timestamp();
        insert_action(txn, action)?;
        insert_op_lite(
            txn,
            op_light,
            op_hash,
            &op_order,
            &timestamp,
            // Using 0 value because this is the cache database and we only need sizes for gossip
            // in the DHT database.
            0,
            todo_no_cache_transfer_data(),
        )?;
        if let Some(status) = validation_status {
            set_validation_status(txn, op_hash, *status)?;
        }
        // We set the integrated to for the cache so it can match the
        // same query as the vault. This can also be used for garbage collection.
        set_when_integrated(txn, op_hash, Timestamp::now())?;
        Ok(())
    }

    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_rendered_ops(txn: &mut Txn<DbKindCache>, ops: &RenderedOps) -> CascadeResult<()> {
        let RenderedOps {
            ops,
            entry,
            warrant,
        } = ops;

        if let Some(warrant) = warrant {
            let op = DhtOpHashed::from_content_sync(warrant.clone());
            insert_op_cache(txn, &op)?;
        }
        if let Some(entry) = entry {
            insert_entry(txn, entry.as_hash(), entry.as_content())?;
        }
        for op in ops {
            Self::insert_rendered_op(txn, op)?;
        }
        Ok(())
    }

    /// Insert a set of agent activity into the Cache.
    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_activity(
        txn: &mut Txn<DbKindCache>,
        ops: Vec<RegisterAgentActivity>,
    ) -> CascadeResult<()> {
        for op in ops {
            let RegisterAgentActivity {
                action:
                    SignedHashed {
                        hashed: HoloHashed { content, .. },
                        signature,
                    },
                ..
            } = op;
            let op =
                DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(signature, content));
            insert_op_cache(txn, &op)?;
            // We set the integrated to for the cache so it can match the
            // same query as the vault. This can also be used for garbage collection.
            set_when_integrated(txn, op.as_hash(), Timestamp::now())?;
        }
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn merge_ops_into_cache(&self, responses: Vec<WireOps>) -> CascadeResult<()> {
        let cache = some_or_return!(self.cache.as_ref());
        cache
            .write_async(|txn| {
                for response in responses {
                    let ops = response.render()?;
                    Self::insert_rendered_ops(txn, &ops)?;
                }
                CascadeResult::Ok(())
            })
            .await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn merge_link_ops_into_cache(
        &self,
        responses: Vec<WireLinkOps>,
        key: WireLinkKey,
    ) -> CascadeResult<()> {
        let cache = some_or_return!(self.cache.as_ref());
        cache
            .write_async(move |txn| {
                for response in responses {
                    let ops = response.render(&key)?;
                    Self::insert_rendered_ops(txn, &ops)?;
                }
                CascadeResult::Ok(())
            })
            .await?;
        Ok(())
    }

    /// Add new activity to the Cache.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
    async fn add_activity_into_cache(
        &self,
        responses: Vec<MustGetAgentActivityResponse>,
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        // Choose a response from all the responses.
        let response = if responses
            .iter()
            .zip(responses.iter().skip(1))
            .all(|(a, b)| a == b)
        {
            // All responses are the same so we can just use the first one.
            responses.into_iter().next()
        } else {
            tracing::info!(
                "Got different must_get_agent_activity responses from different authorities"
            );
            // TODO: Handle conflict.
            // For now try to find one that has got the activity
            responses
                .iter()
                .find(|a| matches!(a, MustGetAgentActivityResponse::Activity { .. }))
                .cloned()
        };

        let cache = some_or_return!(
            self.cache.as_ref(),
            response.unwrap_or(MustGetAgentActivityResponse::IncompleteChain)
        );

        // Commit the activity to the chain.
        match response {
            Some(MustGetAgentActivityResponse::Activity { activity, warrants }) => {
                // TODO: Avoid this clone by committing the ops as references to the db.
                cache
                    .write_async({
                        let activity = activity.clone();
                        let warrants = warrants.clone();
                        move |txn| {
                            Self::insert_activity(txn, activity)?;
                            for warrant in warrants {
                                let op = DhtOpHashed::from_content_sync(warrant);
                                insert_op_cache(txn, &op)?;
                            }

                            CascadeResult::Ok(())
                        }
                    })
                    .await?;
                Ok(MustGetAgentActivityResponse::Activity { activity, warrants })
            }
            Some(response) => Ok(response),
            // Got no responses so the chain is incomplete.
            None => Ok(MustGetAgentActivityResponse::IncompleteChain),
        }
    }

    /// Fetch a Record from the network, caching and returning the results
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn fetch_record(
        &self,
        hash: AnyDhtHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<()> {
        let network = some_or_return!(self.network.as_ref());
        let results = match network
            .get(hash, options)
            .instrument(debug_span!("fetch_record::network_get"))
            .await
        {
            Ok(ops) => ops,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::info!(?e, "No peers to fetch record from");
                vec![]
            }
            Err(e) => return Err(e.into()),
        };

        self.merge_ops_into_cache(results).await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    async fn fetch_links(
        &self,
        link_key: WireLinkKey,
        options: GetLinksRequestOptions,
    ) -> CascadeResult<()> {
        let network = some_or_return!(self.network.as_ref());
        let results = match network.get_links(link_key.clone(), options).await {
            Ok(link_ops) => link_ops,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::debug!(?e, "No peers to fetch links from");
                vec![]
            }
            Err(e) => return Err(e.into()),
        };

        self.merge_link_ops_into_cache(results, link_key.clone())
            .await?;
        Ok(())
    }

    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    async fn fetch_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<Vec<AgentActivityResponse>> {
        let network = some_or_return!(self.network.as_ref(), Vec::with_capacity(0));
        let results = match network.get_agent_activity(agent, query, options).await {
            Ok(response) => response,
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::debug!(?e, "No peers to fetch agent activity from");
                vec![]
            }
            Err(e) => return Err(e.into()),
        };
        Ok(results)
    }

    /// Fetch hash bounded agent activity from the network.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self)))]
    async fn fetch_must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: ChainFilter,
        options: NetworkRequestOptions,
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        tracing::info!(
            "[CASCADE] fetch_must_get_agent_activity: Starting network request for author={:?}, filter={:?}",
            author,
            filter
        );
        let network = some_or_return!(
            self.network.as_ref(),
            MustGetAgentActivityResponse::IncompleteChain
        );
        tracing::info!("[CASCADE] Network available, making request...");
        let results = match network
            .must_get_agent_activity(author, filter, options)
            .await
        {
            Ok(response) => {
                tracing::info!("[CASCADE] Network request succeeded, received {} responses", response.len());
                response
            }
            Err(e @ HolochainP2pError::NoPeersForLocation(_, _)) => {
                tracing::warn!("[CASCADE] No peers to fetch agent activity from: {:?}", e);
                vec![]
            }
            Err(e) => {
                tracing::error!("[CASCADE] Network request failed: {}", e);
                return Err(e.into());
            }
        };

        tracing::info!("[CASCADE] Adding activity into cache...");
        self.add_activity_into_cache(results).await
    }

    /// Get transactions for available databases.
    async fn get_txn_guards(&self) -> CascadeResult<Vec<PTxnGuard>> {
        let mut conns: Vec<_> = Vec::with_capacity(3);
        if let Some(cache) = &self.cache {
            conns.push(cache.get_read_txn().await?);
        }
        if let Some(dht) = &self.dht {
            conns.push(dht.get_read_txn().await?);
        }
        if let Some(authored) = &self.authored {
            conns.push(authored.get_read_txn().await?);
        }
        Ok(conns)
    }

    async fn cascading<Q>(&self, query: Q) -> CascadeResult<Q::Output>
    where
        Q: Query<Item = Judged<SignedActionHashed>> + Send + 'static,
        <Q as Query>::Output: Send + 'static,
    {
        let start = Instant::now();
        let mut txn_guards = self.get_txn_guards().await?;
        let scratch = self.scratch.clone();
        // TODO We may already be on a blocking thread here because this is accessible from a zome call. Ideally we'd have
        //      a way to check this situation and avoid spawning a new thread if we're already on an appropriate thread.
        let results = tokio::task::spawn_blocking(move || {
            let mut txns = Vec::with_capacity(txn_guards.len());
            for conn in &mut txn_guards {
                // TODO The transaction does not actually start here. We're asking for a deferred transaction which is the
                //      right thing to do, but SQLite won't launch that until we do a read operation. If we want a stricter
                //      'point in time' view across databases it might make sense to issue a lightweight read op to each txn here?
                let txn = conn.transaction()?;
                txns.push(txn);
            }
            let txns_ref: Vec<_> = txns.iter().collect();
            let results = match scratch {
                Some(scratch) => scratch
                    .apply_and_then(|scratch| query.run(DbScratch::new(&txns_ref, scratch)))?,
                None => query.run(Txns::from(&txns_ref[..]))?,
            };
            CascadeResult::Ok(results)
        })
        .await??;

        self.duration_metric
            .record(start.elapsed().as_secs_f64(), &[]);

        Ok(results)
    }

    /// Search through the stores and return the first non-none result.
    async fn find_map<F, T>(&self, mut f: F) -> CascadeResult<Option<T>>
    where
        T: Send + 'static,
        F: FnMut(&dyn Store) -> CascadeResult<Option<T>> + Send + Clone + 'static,
    {
        if let Some(cache) = self.cache.clone() {
            let r = cache
                .read_async({
                    let mut f = f.clone();
                    move |raw_txn| f(&CascadeTxnWrapper::from(raw_txn))
                })
                .await?;

            if r.is_some() {
                return Ok(r);
            }
        }

        if let Some(dht) = self.dht.clone() {
            let r = dht
                .read_async({
                    let mut f = f.clone();
                    move |raw_txn| f(&CascadeTxnWrapper::from(raw_txn))
                })
                .await?;

            if r.is_some() {
                return Ok(r);
            }
        }

        if let Some(authored) = self.authored.clone() {
            let r = authored
                .read_async({
                    let mut f = f.clone();
                    move |raw_txn| f(&CascadeTxnWrapper::from(raw_txn))
                })
                .await?;

            if r.is_some() {
                return Ok(r);
            }
        }

        if let Some(scratch) = &self.scratch {
            let r = scratch.apply_and_then(|scratch| f(scratch))?;

            if r.is_some() {
                return Ok(r);
            }
        }

        Ok(None)
    }

    /// Get Entry data along with all CRUD actions associated with it.
    ///
    /// Also returns Rejected actions, which may affect the interpreted validity status of this Entry.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn get_entry_details(
        &self,
        entry_hash: EntryHash,
        options: CascadeOptions,
    ) -> CascadeResult<Option<EntryDetails>> {
        let query: GetEntryDetailsQuery = self.construct_query_with_data_access(entry_hash.clone());

        self.get_latest_with_query(query, entry_hash.into(), options)
            .await
    }

    /// Get the specified Record along with all Updates and Deletes associated with it.
    ///
    /// Can return a Rejected Record.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn get_record_details(
        &self,
        action_hash: ActionHash,
        options: CascadeOptions,
    ) -> CascadeResult<Option<RecordDetails>> {
        let query: GetRecordDetailsQuery =
            self.construct_query_with_data_access(action_hash.clone());

        self.get_latest_with_query(query, action_hash.into(), options)
            .await
    }

    /// Returns the [Record] for this [ActionHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    /// _Note: Deleted actions are a tombstone set_
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn dht_get_action(
        &self,
        action_hash: ActionHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        let query: GetLiveRecordQuery = self.construct_query_with_data_access(action_hash.clone());

        // DESIGN: we can short circuit if we have any local deletes on an action.
        // Is this bad because we will not go back to the network until our
        // cache is cleared. Could someone create an attack based on this fact?

        self.get_local_first_with_query(query, action_hash.into(), options)
            .await
    }

    /// Returns the oldest live [Record] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, options)))]
    pub async fn dht_get_entry(
        &self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        let query: GetLiveEntryQuery = self.construct_query_with_data_access(entry_hash.clone());

        self.get_local_first_with_query(query, entry_hash.into(), options)
            .await
    }

    async fn get_local_first_with_query<Q, O>(
        &self,
        query: Q,
        get_target: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Q::Output>
    where
        Q: Query<Item = Judged<SignedActionHashed>, Output = Option<O>> + Send + 'static,
        O: Send + 'static,
    {
        // Try to get the record from our databases first.
        if let Some(record) = self.cascading(query.clone()).await? {
            return Ok(Some(record));
        }

        if options.strategy == GetStrategy::Network {
            // If we are allowed to get the data from the network then try to retrieve the missing data.
            self.get_latest_with_query(
                query,
                get_target,
                CascadeOptions {
                    network_request_options: NetworkRequestOptions::default(),
                    get_options: options,
                },
            )
            .await
        } else {
            // We're not allowed to get the data from the network, and it's not stored locally so
            // just return None.
            Ok(None)
        }
    }

    async fn get_latest_with_query<Q, O>(
        &self,
        query: Q,
        get_target: AnyDhtHash,
        options: CascadeOptions,
    ) -> CascadeResult<Q::Output>
    where
        Q: Query<Item = Judged<SignedActionHashed>, Output = Option<O>> + Send + 'static,
        O: Send + 'static,
    {
        // If we are allowed to get the data from the network then try to retrieve the latest data.
        if options.get_options.strategy == GetStrategy::Network {
            // If we are not in the process of authoring this hash or its
            // authority we need a network call.
            let authoring = self.am_i_authoring(&get_target)?;
            let authority = self.am_i_an_authority(get_target.clone().into()).await?;
            if !(authoring || authority) {
                // Fetch the data if there is anyone to fetch it from.
                match self
                    .fetch_record(get_target, options.network_request_options)
                    .await
                {
                    Ok(_) => (),
                    Err(CascadeError::NetworkError(
                        e @ HolochainP2pError::NoPeersForLocation(_, _),
                    )) => {
                        tracing::debug!(?e, "No peers to fetch record from");
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }

        // Either the cache was updated by the network fetch, or just get what was already
        // available from the cache.
        self.cascading(query).await
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
                        network_request_options: NetworkRequestOptions::default(),
                        get_options: options,
                    },
                )
                .await?
                .map(Details::Entry)),
            AnyDhtHashPrimitive::Action(hash) => Ok(self
                .get_record_details(
                    hash,
                    CascadeOptions {
                        network_request_options: NetworkRequestOptions::default(),
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
        // only fetch links from the network if I am not an authority and
        // GetStrategy is Network
        if let GetStrategy::Network = options.get_options.strategy {
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

        let query = GetLinksQuery::new(
            key.base,
            key.type_query,
            key.tag,
            GetLinksFilter {
                after: key.after,
                before: key.before,
                author: key.author,
            },
        );

        self.cascading(query).await
    }

    /// Return all CreateLink actions and DeleteLink actions ordered by time.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, key, options)))]
    pub async fn get_links_details(
        &self,
        key: WireLinkKey,
        options: GetLinksRequestOptions,
    ) -> CascadeResult<Vec<(SignedActionHashed, Vec<SignedActionHashed>)>> {
        // only fetch link details from network if i am not an authority and
        // GetStrategy is Network
        if let GetStrategy::Network = options.get_options.strategy {
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
        let query = GetLinkDetailsQuery::new(key.base, key.type_query, key.tag);
        self.cascading(query).await
    }

    /// Count the number of links matching the `query`.
    #[cfg_attr(feature = "instrument", tracing::instrument(skip(self, query)))]
    pub async fn dht_count_links(&self, query: WireLinkQuery) -> CascadeResult<usize> {
        let mut links = HashSet::<ActionHash>::new();
        if !self.am_i_an_authority(query.base.clone()).await? {
            if let Some(network) = &self.network {
                match network
                    .count_links(query.clone(), NetworkRequestOptions::default())
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

        let get_links_query = GetLinksQuery::new(
            query.base.clone(),
            query.link_type.clone(),
            query.tag_prefix.clone(),
            query.into(),
        );

        links.extend(
            self.cascading(get_links_query)
                .await?
                .into_iter()
                .map(|l| l.create_link_hash),
        );

        Ok(links.len())
    }

    /// Request a hash bounded chain query.
    pub async fn must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: ChainFilter,
        options: NetworkRequestOptions,
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        tracing::info!(
            "[CASCADE] must_get_agent_activity: author={:?}, filter={:?}",
            author,
            filter
        );
        // Get the available databases.
        let mut txn_guards = self.get_txn_guards().await?;
        tracing::info!("[CASCADE] Got {} transaction guards", txn_guards.len());
        let scratch = self.scratch.clone();

        // For each store try to get the bounded activity.
        let results = tokio::task::spawn_blocking({
            let author = author.clone();
            let filter = filter.clone();
            let scratch = scratch.clone();
            move || {
                let mut results = Vec::with_capacity(txn_guards.len() + 1);
                for txn_guard in &mut txn_guards {
                    let txn = txn_guard.transaction()?;
                    let r = match &scratch {
                        Some(scratch) => {
                            scratch.apply_and_then(|scratch| {
                                authority::get_agent_activity_query::must_get_agent_activity::get_bounded_activity(&txn, Some(scratch), &author, filter.clone())
                            })?
                        }
                        None => authority::get_agent_activity_query::must_get_agent_activity::get_bounded_activity(&txn, None, &author, filter.clone())?
                    };
                    results.push(r);
                }
                CascadeResult::Ok(results)
            }
        })
            .await??;

        tracing::info!("[CASCADE] Got {} results from local stores", results.len());
        let merged_response =
            holochain_types::chain::merge_bounded_agent_activity_responses(results);
        let result =
            authority::get_agent_activity_query::must_get_agent_activity::filter_then_check(
                merged_response,
            );

        tracing::info!("[CASCADE] Filtered result type: {:?}", std::mem::discriminant(&result));
        // Short circuit if we have a result.
        if matches!(result, MustGetAgentActivityResponse::Activity { .. }) {
            tracing::info!("[CASCADE] Found activity locally, returning early");
            return Ok(result);
        }

        // If we are the authority then don't go to the network.
        let i_am_authority = self.am_i_an_authority(author.clone().into()).await?;
        tracing::info!("[CASCADE] Am I authority: {}", i_am_authority);
        if i_am_authority {
            // If I am an authority and I didn't get a result before
            // this point then the chain is incomplete for this request.
            tracing::info!("[CASCADE] I am authority and chain is incomplete, returning IncompleteChain");
            Ok(MustGetAgentActivityResponse::IncompleteChain)
        } else {
            tracing::info!("[CASCADE] Not authority, fetching from network...");
            let result = self
                .fetch_must_get_agent_activity(author.clone(), filter, options)
                .await?;
            // Add warrants received from the network to the scratch space to be written
            // to the DHT database.
            if let MustGetAgentActivityResponse::Activity { warrants, .. } = &result {
                if let Some(scratch) = scratch {
                    if let Err(err) = scratch.apply(|scratch| {
                        for warrant in warrants.iter() {
                            scratch.add_warrant(SignedWarrant::new(
                                warrant.data().clone(),
                                warrant.signature().clone(),
                            ));
                        }
                    }) {
                        tracing::warn!(
                            ?err,
                            "Failed to add warrants from network response to scratch"
                        );
                    }
                }
            }
            Ok(result)
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
        let status_only = !(options.include_valid_activity || options.include_rejected_activity);

        // If we're an authority then we allow local queries. This means we consider ourselves an authority
        // for the agent in question. If the options specify network, for example because we are looking for
        // warrants we don't know about or for countersigning actions, then we will go to the network
        // regardless of authority status.
        let authority = self.am_i_an_authority(agent.clone().into()).await?;

        let merged_response = if authority && options.get_options.strategy == GetStrategy::Local {
            match self.dht.clone() {
                Some(vault) => {
                    authority::handle_get_agent_activity(
                        vault,
                        agent.clone(),
                        query.clone(),
                        (&options).into(),
                    )
                    .await?
                }
                None => {
                    info!("Unable to get agent activity because this cascade does not have DHT access");
                    agent_activity::merge_activities(
                        agent.clone(),
                        &options,
                        Vec::with_capacity(0),
                    )?
                }
            }
        } else {
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
                    RecordEntry::NotStored => r.action().entry_hash().map(|h| h.clone().into()),
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
                        .signed_action()
                        .action()
                        .entry_hash()
                        .map(|entry_hash| (entry_hash, &r.entry)),
                    None => None,
                })
                .collect::<HashMap<_, _>>();

            match &mut chain_items {
                ChainItems::Full(records) => {
                    for record in records.iter_mut() {
                        if let RecordEntry::NotStored = record.entry {
                            if let Some(entry_hash) = record.action().entry_hash() {
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

    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn am_i_authoring(&self, hash: &AnyDhtHash) -> CascadeResult<bool> {
        let scratch = some_or_return!(self.scratch.as_ref(), false);
        Ok(scratch.apply_and_then(|scratch| scratch.contains_hash(hash))?)
    }

    async fn am_i_an_authority(&self, hash: OpBasis) -> CascadeResult<bool> {
        let network = some_or_return!(self.network.as_ref(), false);
        Ok(network.authority_for_hash(hash).await?)
    }

    /// Construct a query with private data access if this cascade has been
    /// constructed with private data access.
    fn construct_query_with_data_access<H, Q: PrivateDataQuery<Hash = H>>(&self, hash: H) -> Q {
        match self.private_data.clone() {
            Some(author) => Q::with_private_data_access(hash, author),
            None => Q::without_private_data_access(hash),
        }
    }
}

/// TODO
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
        let private_data = self.private_data.clone();
        let result = self
            .find_map({
                let hash = hash.clone();
                move |store| {
                    Ok(store.get_public_or_authored_entry(
                        &hash,
                        private_data.as_ref().map(|a| a.as_ref()),
                    )?)
                }
            })
            .await?;
        if result.is_some() {
            return Ok(result.map(|e| (EntryHashed::from_content_sync(e), CascadeSource::Local)));
        }
        self.fetch_record(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let private_data = self.private_data.clone();
        let result = self
            .find_map({
                let hash = hash.clone();
                move |store| {
                    Ok(store.get_public_or_authored_entry(
                        &hash,
                        private_data.as_ref().map(|a| a.as_ref()),
                    )?)
                }
            })
            .await?;
        Ok(result.map(|e| (EntryHashed::from_content_sync(e), CascadeSource::Network)))
    }

    async fn retrieve_action(
        &self,
        hash: ActionHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(SignedActionHashed, CascadeSource)>> {
        let result = self
            .find_map({
                let hash = hash.clone();
                move |store| Ok(store.get_action(&hash)?)
            })
            .await?;
        if result.is_some() {
            return Ok(result.map(|a| (a, CascadeSource::Local)));
        }
        self.fetch_record(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let result = self
            .find_map(move |store| {
                Ok(store
                    .get_action(&hash)?
                    .map(|a| (a, CascadeSource::Network)))
            })
            .await?;
        Ok(result)
    }

    async fn retrieve_public_record(
        &self,
        hash: AnyDhtHash,
        options: NetworkRequestOptions,
    ) -> CascadeResult<Option<(Record, CascadeSource)>> {
        let result = self
            .find_map({
                let hash = hash.clone();
                move |store| Ok(store.get_public_record(&hash)?)
            })
            .await?;
        if result.is_some() {
            return Ok(result.map(|r| (r, CascadeSource::Local)));
        }
        self.fetch_record(hash.clone(), options).await?;

        // Check if we have the data now after the network call.
        let result = self
            .find_map(move |store| Ok(store.get_public_record(&hash)?))
            .await?;
        Ok(result.map(|r| (r, CascadeSource::Network)))
    }
}

#[cfg(feature = "test_utils")]
impl MockCascade {
    /// Construct a mock which acts as if the given records were part of local storage
    pub fn with_records(records: Vec<Record>) -> Self {
        let mut cascade = Self::default();

        let map: HashMap<AnyDhtHash, Record> = records
            .into_iter()
            .flat_map(|r| {
                let mut items = vec![(r.action_address().clone().into(), r.clone())];
                if let Some(eh) = r.action().entry_hash() {
                    items.push((eh.clone().into(), r))
                }
                items
            })
            .collect();

        let map0 = Arc::new(parking_lot::Mutex::new(map));

        let map = map0.clone();
        cascade
            .expect_retrieve_public_record()
            .returning(move |hash, _| {
                let m = map.lock();
                let result = m.get(&hash).map(|r| (r.clone(), CascadeSource::Local));
                Box::pin(async move { Ok(result) })
            });

        let map = map0.clone();
        cascade.expect_retrieve_action().returning(move |hash, _| {
            let m = map.lock();
            let result = m
                .get(&hash.into())
                .map(|r| (r.signed_action().clone(), CascadeSource::Local));
            Box::pin(async move { Ok(result) })
        });

        let map = map0;
        cascade.expect_retrieve_entry().returning(move |hash, _| {
            let m = map.lock();
            let result = m.get(&hash.into()).map(|r| {
                (
                    EntryHashed::from_content_sync(r.entry().as_option().unwrap().clone()),
                    CascadeSource::Local,
                )
            });
            Box::pin(async move { Ok(result) })
        });

        cascade
    }
}

#[tokio::test]
async fn test_mock_cascade_with_records() {
    use ::fixt::fixt;
    let records = vec![fixt!(Record), fixt!(Record), fixt!(Record)];
    let cascade = MockCascade::with_records(records.clone());
    let opts = NetworkRequestOptions::default();
    let (r0, _) = cascade
        .retrieve_public_record(records[0].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r1, _) = cascade
        .retrieve_public_record(records[1].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r2, _) = cascade
        .retrieve_public_record(records[2].action_address().clone().into(), opts)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(records, vec![r0, r1, r2]);
}
