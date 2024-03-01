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
//!     before returning the data, so for instance, Deletes
//!     are allowed to annihilate Creates so that neither is returned. This is a more
//!     "refined" form of fetching data.
//! - "retrieve" only fetches the data if it exists, without regard to validation status.
//!     This is a more "raw" form of fetching data.
//!
#![warn(missing_docs)]

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use error::CascadeResult;
use holo_hash::ActionHash;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
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
use tracing::*;

#[cfg(feature = "test_utils")]
use kitsune_p2p::dependencies::kitsune_p2p_types::box_fut_plain;
#[cfg(feature = "test_utils")]
use kitsune_p2p::dependencies::kitsune_p2p_types::tx2::tx2_utils::ShareOpen;
#[cfg(feature = "test_utils")]
use std::collections::HashMap;

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

/// The Cascade is a multi-tiered accessor for Holochain DHT data.
///
/// See the module-level docs for more info.
#[derive(Clone)]
pub struct CascadeImpl<Network: Send + Sync = HolochainP2pDna> {
    authored: Option<DbRead<DbKindAuthored>>,
    dht: Option<DbRead<DbKindDht>>,
    cache: Option<DbWrite<DbKindCache>>,
    scratch: Option<SyncScratch>,
    network: Option<Network>,
    private_data: Option<Arc<AgentPubKey>>,
    duration_metric: &'static CascadeDurationMetric,
}

impl<Network> CascadeImpl<Network>
where
    Network: HolochainP2pDnaT + Clone + 'static + Send + Sync,
{
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
    pub fn with_network<N: HolochainP2pDnaT>(
        self,
        network: N,
        cache_db: DbWrite<DbKindCache>,
    ) -> CascadeImpl<N> {
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
}

impl CascadeImpl<HolochainP2pDna> {
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
    pub fn from_workspace_and_network<N, AuthorDb, DhtDb>(
        workspace: &HostFnWorkspace<AuthorDb, DhtDb>,
        network: N,
    ) -> CascadeImpl<N>
    where
        N: HolochainP2pDnaT + Clone,
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
        CascadeImpl::<N> {
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
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<(EntryHashed, CascadeSource)>>;

    /// Retrieve [`SignedActionHashed`] from either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    async fn retrieve_action(
        &self,
        hash: ActionHash,
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<(SignedActionHashed, CascadeSource)>>;

    /// Retrieve data from either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    async fn retrieve(
        &self,
        hash: AnyDhtHash,
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<(Record, CascadeSource)>>;
}

#[async_trait::async_trait]
impl<Network> Cascade for CascadeImpl<Network>
where
    Network: HolochainP2pDnaT + Clone + 'static + Send,
{
    async fn retrieve_entry(
        &self,
        hash: EntryHash,
        mut options: NetworkGetOptions,
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
        options.request_type = holochain_p2p::event::GetRequest::Pending;
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
        mut options: NetworkGetOptions,
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
        options.request_type = holochain_p2p::event::GetRequest::Pending;
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

    async fn retrieve(
        &self,
        hash: AnyDhtHash,
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<(Record, CascadeSource)>> {
        let private_data = self.private_data.clone();
        let result = self
            .find_map({
                let hash = hash.clone();
                move |store| {
                    Ok(store.get_public_or_authored_record(
                        &hash,
                        private_data.as_ref().map(|a| a.as_ref()),
                    )?)
                }
            })
            .await?;
        if result.is_some() {
            return Ok(result.map(|r| (r, CascadeSource::Local)));
        }
        options.request_type = holochain_p2p::event::GetRequest::Pending;
        self.fetch_record(hash.clone(), options).await?;

        let private_data = self.private_data.clone();
        // Check if we have the data now after the network call.
        let result = self
            .find_map(move |store| {
                Ok(store.get_public_or_authored_record(
                    &hash,
                    private_data.as_ref().map(|a| a.as_ref()),
                )?)
            })
            .await?;
        Ok(result.map(|r| (r, CascadeSource::Network)))
    }
}

impl<Network> CascadeImpl<Network>
where
    Network: HolochainP2pDnaT + Clone + 'static + Send,
{
    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_rendered_op(txn: &mut Transaction, op: &RenderedOp) -> CascadeResult<()> {
        let RenderedOp {
            op_light,
            op_hash,
            action,
            validation_status,
        } = op;
        let op_order = OpOrder::new(op_light.get_type(), action.action().timestamp());
        let timestamp = action.action().timestamp();
        insert_action(txn, action)?;
        insert_op_lite(txn, op_light, op_hash, &op_order, &timestamp)?;
        if let Some(status) = validation_status {
            set_validation_status(txn, op_hash, *status)?;
        }
        // We set the integrated to for the cache so it can match the
        // same query as the vault. This can also be used for garbage collection.
        set_when_integrated(txn, op_hash, Timestamp::now())?;
        Ok(())
    }

    #[allow(clippy::result_large_err)] // TODO - investigate this lint
    fn insert_rendered_ops(txn: &mut Transaction, ops: &RenderedOps) -> CascadeResult<()> {
        let RenderedOps { ops, entry } = ops;
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
        txn: &mut Transaction,
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
                DhtOpHashed::from_content_sync(DhtOp::RegisterAgentActivity(signature, content));
            insert_op(txn, &op)?;
            // We set the integrated to for the cache so it can match the
            // same query as the vault. This can also be used for garbage collection.
            set_when_integrated(txn, op.as_hash(), Timestamp::now())?;
        }
        Ok(())
    }

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
            // For now try to find one that has got the activity.
            responses
                .into_iter()
                .find(|a| matches!(a, MustGetAgentActivityResponse::Activity(_)))
        };

        let cache = some_or_return!(
            self.cache.as_ref(),
            response.unwrap_or(MustGetAgentActivityResponse::IncompleteChain)
        );

        // Commit the activity to the chain.
        match response {
            Some(MustGetAgentActivityResponse::Activity(activity)) => {
                // TODO: Avoid this clone by committing the ops as references to the db.
                cache
                    .write_async({
                        let activity = activity.clone();
                        move |txn| {
                            Self::insert_activity(txn, activity)?;
                            CascadeResult::Ok(())
                        }
                    })
                    .await?;
                Ok(MustGetAgentActivityResponse::Activity(activity))
            }
            Some(response) => Ok(response),
            // Got no responses so the chain is incomplete.
            None => Ok(MustGetAgentActivityResponse::IncompleteChain),
        }
    }

    /// Fetch a Record from the network, caching and returning the results
    #[instrument(skip(self, options))]
    pub async fn fetch_record(
        &self,
        hash: AnyDhtHash,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        let network = some_or_return!(self.network.as_ref());
        let results = network
            .get(hash, options.clone())
            .instrument(debug_span!("fetch_record::network_get"))
            .await?;

        self.merge_ops_into_cache(results).await?;
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_links(
        &self,
        link_key: WireLinkKey,
        options: GetLinksOptions,
    ) -> CascadeResult<()> {
        let network = some_or_return!(self.network.as_ref());
        let results = network.get_links(link_key.clone(), options).await?;

        self.merge_link_ops_into_cache(results, link_key.clone())
            .await?;
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<Vec<AgentActivityResponse<ActionHash>>> {
        let network = some_or_return!(self.network.as_ref(), Vec::with_capacity(0));
        Ok(network.get_agent_activity(agent, query, options).await?)
    }

    #[instrument(skip(self))]
    /// Fetch hash bounded agent activity from the network.
    async fn fetch_must_get_agent_activity(
        &self,
        author: AgentPubKey,
        filter: holochain_zome_types::chain::ChainFilter,
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        let network = some_or_return!(
            self.network.as_ref(),
            MustGetAgentActivityResponse::IncompleteChain
        );
        let results = network.must_get_agent_activity(author, filter).await?;

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
                    move |raw_txn| f(&Txn::from(&raw_txn))
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
                    move |raw_txn| f(&Txn::from(&raw_txn))
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
                    move |raw_txn| f(&Txn::from(&raw_txn))
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
    #[instrument(skip(self, options))]
    pub async fn get_entry_details(
        &self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<EntryDetails>> {
        let query: GetEntryDetailsQuery = self.construct_query_with_data_access(entry_hash.clone());
        if let GetStrategy::Local = options.strategy {
            // Only return what is in the database.
            return self.cascading(query.clone()).await;
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        let authoring = self.am_i_authoring(&entry_hash.clone().into())?;
        let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
        if !(authoring || authority) {
            self.fetch_record(entry_hash.into(), options.into()).await?;
        }

        // Check if we have the data now after the network call.
        self.cascading(query).await
    }

    /// Get the specified Record along with all Updates and Deletes associated with it.
    ///
    /// Can return a Rejected Record.
    #[instrument(skip(self, options))]
    pub async fn get_record_details(
        &self,
        action_hash: ActionHash,
        options: GetOptions,
    ) -> CascadeResult<Option<RecordDetails>> {
        let query: GetRecordDetailsQuery =
            self.construct_query_with_data_access(action_hash.clone());

        // DESIGN: we can short circuit if we have any local deletes on an action.
        // Is this bad because we will not go back to the network until our
        // cache is cleared. Could someone create an attack based on this fact?

        if let GetStrategy::Local = options.strategy {
            // Only return what is in the database.
            return self.cascading(query.clone()).await;
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        let authoring = self.am_i_authoring(&action_hash.clone().into())?;
        let authority = self.am_i_an_authority(action_hash.clone().into()).await?;
        if !(authoring || authority) {
            self.fetch_record(action_hash.into(), options.into())
                .await?;
        }

        // Check if we have the data now after the network call.
        self.cascading(query).await
    }

    #[instrument(skip(self, options))]
    /// Returns the [Record] for this [ActionHash] if it is live
    /// by getting the latest available metadata from authorities
    /// combined with this agents authored data.
    /// _Note: Deleted actions are a tombstone set_
    pub async fn dht_get_action(
        &self,
        action_hash: ActionHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        let query: GetLiveRecordQuery = self.construct_query_with_data_access(action_hash.clone());

        // DESIGN: we can short circuit if we have any local deletes on an action.
        // Is this bad because we will not go back to the network until our
        // cache is cleared. Could someone create an attack based on this fact?

        if let GetStrategy::Local = options.strategy {
            // Only return what is in the database.
            return self.cascading(query.clone()).await;
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        let authoring = self.am_i_authoring(&action_hash.clone().into())?;
        let authority = self.am_i_an_authority(action_hash.clone().into()).await?;
        if !(authoring || authority) {
            self.fetch_record(action_hash.into(), options.into())
                .await?;
        }

        // Check if we have the data now after the network call.
        self.cascading(query).await
    }

    #[instrument(skip(self, options))]
    /// Returns the oldest live [Record] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    pub async fn dht_get_entry(
        &self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Record>> {
        let query: GetLiveEntryQuery = self.construct_query_with_data_access(entry_hash.clone());

        if let GetStrategy::Local = options.strategy {
            // Only return what is in the database.
            return self.cascading(query.clone()).await;
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        let authoring = self.am_i_authoring(&entry_hash.clone().into())?;
        let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
        if !(authoring || authority) {
            self.fetch_record(entry_hash.into(), options.into()).await?;
        }

        // Check if we have the data now after the network call.
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

    #[instrument(skip(self))]
    /// Updates the cache with the latest network authority data
    /// and returns what is in the cache.
    /// This gives you the latest possible picture of the current dht state.
    /// Data from your zome call is also added to the cache.
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
    #[instrument(skip(self))]
    pub async fn get_details(
        &self,
        hash: AnyDhtHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Details>> {
        match hash.into_primitive() {
            AnyDhtHashPrimitive::Entry(hash) => Ok(self
                .get_entry_details(hash, options)
                .await?
                .map(Details::Entry)),
            AnyDhtHashPrimitive::Action(hash) => Ok(self
                .get_record_details(hash, options)
                .await?
                .map(Details::Record)),
        }
    }

    #[instrument(skip(self, options))]
    /// Gets an links from the cas or cache depending on it's metadata
    // The default behavior is to skip deleted or replaced entries.
    pub async fn dht_get_links(
        &self,
        key: WireLinkKey,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<Link>> {
        // only fetch links from network if i am not an authority and
        // GetStrategy is Latest
        if let GetStrategy::Network = options.get_options.strategy {
            let authority = self.am_i_an_authority(key.base.clone()).await?;
            if !authority {
                self.fetch_links(key.clone(), options).await?;
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

    #[instrument(skip(self, key, options))]
    /// Return all CreateLink actions
    /// and DeleteLink actions ordered by time.
    pub async fn get_link_details(
        &self,
        key: WireLinkKey,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<(SignedActionHashed, Vec<SignedActionHashed>)>> {
        // only fetch link details from network if i am not an authority and
        // GetStrategy is Network
        if let GetStrategy::Network = options.get_options.strategy {
            let authority = self.am_i_an_authority(key.base.clone()).await?;
            if !authority {
                self.fetch_links(key.clone(), options).await?;
            }
        }
        let query = GetLinkDetailsQuery::new(key.base, key.type_query, key.tag);
        self.cascading(query).await
    }

    /// Count the number of links matching the `query`.
    #[instrument(skip(self, query))]
    pub async fn dht_count_links(&self, query: WireLinkQuery) -> CascadeResult<usize> {
        let mut links = HashSet::<ActionHash>::new();
        if !self.am_i_an_authority(query.base.clone()).await? {
            if let Some(network) = &self.network {
                links.extend(
                    network
                        .count_links(query.clone())
                        .await?
                        .create_link_actions(),
                );
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
    ) -> CascadeResult<MustGetAgentActivityResponse> {
        // Get the available databases.
        let mut txn_guards = self.get_txn_guards().await?;
        let scratch = self.scratch.clone();

        // For each store try to get the bounded activity.
        let results = tokio::task::spawn_blocking({
            let author = author.clone();
            let filter = filter.clone();
            move || {
                let mut results = Vec::with_capacity(txn_guards.len() + 1);
                for txn_guard in &mut txn_guards {
                    let mut txn = txn_guard.transaction()?;
                    let r = match &scratch {
                        Some(scratch) => {
                            scratch.apply_and_then(|scratch| {
                                authority::get_agent_activity_query::must_get_agent_activity::get_bounded_activity(&mut txn, Some(scratch), &author, filter.clone())
                            })?
                        }
                        None => authority::get_agent_activity_query::must_get_agent_activity::get_bounded_activity(&mut txn, None, &author, filter.clone())?
                    };
                    results.push(r);
                }
                CascadeResult::Ok(results)
            }
        })
            .await??;

        let merged_results = results.iter().fold(
            // It's sort of arbitrary what the initial value is as long as it's
            // not an activity response.
            BoundedMustGetAgentActivityResponse::EmptyRange,
            holochain_types::chain::merge_bounded_agent_activity_responses,
        );

        let result =
            authority::get_agent_activity_query::must_get_agent_activity::filter_then_check(
                merged_results,
            );

        // Short circuit if we have a result.
        if matches!(result, MustGetAgentActivityResponse::Activity(_)) {
            return Ok(result);
        }

        // If we are the authority then don't go to the network.
        let i_am_authority = self.am_i_an_authority(author.clone().into()).await?;
        if i_am_authority {
            // If I am an authority and I didn't get a result before
            // this point then the chain is incomplete for this request.
            Ok(MustGetAgentActivityResponse::IncompleteChain)
        } else {
            self.fetch_must_get_agent_activity(author.clone(), filter)
                .await
        }
    }

    #[instrument(skip(self, agent, query, options))]
    /// Get agent activity from agent activity authorities.
    /// Hashes are requested from the authority and cache for valid chains.
    /// Options:
    /// - include_valid_activity will include the valid chain hashes.
    /// - include_rejected_activity will include the invalid chain hashes.
    /// - include_full_actions will fetch the valid actions in parallel (requires include_valid_activity)
    /// Query:
    /// - include_entries will also fetch the entries in parallel (requires include_full_actions)
    /// - sequence_range will get all the activity in the exclusive range
    /// - action_type and entry_type will filter the activity (requires include_full_actions)
    pub async fn get_agent_activity(
        &self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse<Record>> {
        let status_only = !options.include_rejected_activity && !options.include_valid_activity;
        // DESIGN: Evaluate if it's ok to **not** go to another authority for agent activity?
        let authority = self.am_i_an_authority(agent.clone().into()).await?;
        let merged_response = if !authority {
            let results = self
                .fetch_agent_activity(agent.clone(), query.clone(), options.clone())
                .await?;
            let merged_response: AgentActivityResponse<ActionHash> =
                agent_activity::merge_activities(agent.clone(), &options, results)?;
            merged_response
        } else {
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
                None => agent_activity::merge_activities(
                    agent.clone(),
                    &options,
                    Vec::with_capacity(0),
                )?,
            }
        };

        // If the response is empty we can finish.
        if let ChainStatus::Empty = &merged_response.status {
            return Ok(AgentActivityResponse::from_empty(merged_response));
        }

        // If the request is just for the status then return.
        if status_only {
            return Ok(AgentActivityResponse::status_only(merged_response));
        }

        // If they don't want the full actions then just return the hashes.
        if !options.include_full_actions {
            return Ok(AgentActivityResponse::hashes_only(merged_response));
        }

        // If they need the full actions then we will do concurrent gets.
        let AgentActivityResponse {
            agent,
            valid_activity,
            rejected_activity,
            status,
            highest_observed,
        } = merged_response;
        let valid_activity = match valid_activity {
            ChainItems::Hashes(hashes) => {
                // If we can't get one of the actions then don't return any.
                // DESIGN: Is this the correct choice?
                let maybe_chain: Option<Vec<_>> = self
                    .get_concurrent(
                        hashes.into_iter().map(|(_, h)| h.into()),
                        GetOptions::local(),
                    )
                    .await?
                    .into_iter()
                    .collect();
                match maybe_chain {
                    Some(mut chain) => {
                        chain.sort_unstable_by_key(|el| el.action().action_seq());
                        ChainItems::Full(chain)
                    }
                    None => ChainItems::Full(Vec::with_capacity(0)),
                }
            }
            ChainItems::Full(_) => ChainItems::Full(Vec::with_capacity(0)),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        let rejected_activity = match rejected_activity {
            ChainItems::Hashes(hashes) => {
                // If we can't get one of the actions then don't return any.
                // DESIGN: Is this the correct choice?
                let maybe_chain: Option<Vec<_>> = self
                    .get_concurrent(
                        hashes.into_iter().map(|(_, h)| h.into()),
                        GetOptions::local(),
                    )
                    .await?
                    .into_iter()
                    .collect();
                match maybe_chain {
                    Some(mut chain) => {
                        chain.sort_unstable_by_key(|el| el.action().action_seq());
                        ChainItems::Full(chain)
                    }
                    None => ChainItems::Full(Vec::with_capacity(0)),
                }
            }
            ChainItems::Full(_) => ChainItems::Full(Vec::with_capacity(0)),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };

        let r = AgentActivityResponse {
            agent,
            valid_activity,
            rejected_activity,
            status,
            highest_observed,
        };
        Ok(r)
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

        let map0 = ShareOpen::new(map);

        let map = map0.clone();
        cascade.expect_retrieve().returning(move |hash, _| {
            box_fut_plain(Ok(map.share_ref(|m| {
                m.get(&hash).map(|r| (r.clone(), CascadeSource::Local))
            })))
        });

        let map = map0.clone();
        cascade.expect_retrieve_action().returning(move |hash, _| {
            box_fut_plain(Ok(map.share_ref(|m| {
                m.get(&hash.into())
                    .map(|r| (r.signed_action().clone(), CascadeSource::Local))
            })))
        });

        let map = map0;
        cascade.expect_retrieve_entry().returning(move |hash, _| {
            box_fut_plain(Ok(map.share_ref(|m| {
                m.get(&hash.into()).map(|r| {
                    (
                        EntryHashed::from_content_sync(r.entry().as_option().unwrap().clone()),
                        CascadeSource::Local,
                    )
                })
            })))
        });

        cascade
    }
}

#[tokio::test]
async fn test_mock_cascade_with_records() {
    use ::fixt::fixt;
    let records = vec![fixt!(Record), fixt!(Record), fixt!(Record)];
    let cascade = MockCascade::with_records(records.clone());
    let opts = NetworkGetOptions::default();
    let (r0, _) = cascade
        .retrieve(records[0].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r1, _) = cascade
        .retrieve(records[1].action_address().clone().into(), opts.clone())
        .await
        .unwrap()
        .unwrap();
    let (r2, _) = cascade
        .retrieve(records[2].action_address().clone().into(), opts)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(records, vec![r0, r1, r2]);
}
