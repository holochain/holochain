//! # Cascade
//! ## Retrieve vs Get
//! Get checks CRUD metadata before returning an the data
//! where as retrieve only checks that where the data was found
//! the appropriate validation has been run.

use std::sync::Arc;

use error::CascadeResult;
use holo_hash::hash_type::AnyDht;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::HolochainP2pDna;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::rusqlite::Transaction;
use holochain_state::host_fn_workspace::HostFnStores;
use holochain_state::host_fn_workspace::HostFnWorkspace;
use holochain_state::mutations::set_validation_status;
use holochain_state::prelude::*;
use holochain_state::query::element_details::GetElementDetailsQuery;
use holochain_state::query::entry_details::GetEntryDetailsQuery;
use holochain_state::query::link::GetLinksQuery;
use holochain_state::query::link_details::GetLinkDetailsQuery;
use holochain_state::query::live_element::GetLiveElementQuery;
use holochain_state::query::live_entry::GetLiveEntryQuery;
use holochain_state::query::DbScratch;
use holochain_state::query::StateQueryError;
use holochain_state::scratch::SyncScratch;
use holochain_types::prelude::*;
use mutations::insert_entry;
use mutations::insert_header;
use mutations::insert_op_lite;
use tracing::*;

pub mod authority;
pub mod error;

mod agent_activity;

#[cfg(any(test, feature = "test_utils"))]
pub mod test_utils;

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

#[derive(Clone)]
pub struct Cascade<Network = HolochainP2pDna> {
    authored: Option<DbRead<DbKindAuthored>>,
    dht: Option<DbRead<DbKindDht>>,
    cache: Option<DbWrite<DbKindCache>>,
    scratch: Option<SyncScratch>,
    network: Option<Network>,
    private_data: Option<Arc<AgentPubKey>>,
}

impl<Network> Cascade<Network>
where
    Network: HolochainP2pDnaT + Clone + 'static + Send,
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
    // TODO: We do want to be able to use the cache without
    // the network but we always need a cache when we have a
    // network. Perhaps this can be proven at the type level?
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
    pub fn with_network<N: HolochainP2pDnaT + Clone>(
        self,
        network: N,
        cache_env: DbWrite<DbKindCache>,
    ) -> Cascade<N> {
        Cascade {
            authored: self.authored,
            dht: self.dht,
            scratch: self.scratch,
            private_data: self.private_data,
            cache: Some(cache_env),
            network: Some(network),
        }
    }
}
impl Cascade<HolochainP2pDna> {
    /// Constructs an empty [Cascade].
    pub fn empty() -> Self {
        Self {
            authored: None,
            dht: None,
            network: None,
            cache: None,
            scratch: None,
            private_data: None,
        }
    }

    pub fn from_workspace_network<N, AuthorDb, DhtDb>(
        workspace: &HostFnWorkspace<AuthorDb, DhtDb>,
        network: N,
    ) -> Cascade<N>
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
        Cascade::<N> {
            authored: Some(authored),
            dht: Some(dht),
            cache: Some(cache),
            private_data,
            scratch,
            network: Some(network),
        }
    }
    pub fn from_workspace(stores: HostFnStores, author: Option<Arc<AgentPubKey>>) -> Self {
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
        }
    }
}

impl<Network> Cascade<Network>
where
    Network: HolochainP2pDnaT + Clone + 'static + Send,
{
    fn insert_rendered_op(txn: &mut Transaction, op: RenderedOp) -> CascadeResult<()> {
        let RenderedOp {
            op_light,
            op_hash,
            header,
            validation_status,
        } = op;
        let op_order = OpOrder::new(op_light.get_type(), header.header().timestamp());
        let timestamp = header.header().timestamp();
        insert_header(txn, header)?;
        insert_op_lite(txn, op_light, op_hash.clone(), op_order, timestamp)?;
        if let Some(status) = validation_status {
            set_validation_status(txn, op_hash.clone(), status)?;
        }
        // We set the integrated to for the cache so it can match the
        // same query as the vault. This can also be used for garbage collection.
        set_when_integrated(txn, op_hash, Timestamp::now())?;
        Ok(())
    }

    fn insert_rendered_ops(txn: &mut Transaction, ops: RenderedOps) -> CascadeResult<()> {
        let RenderedOps { ops, entry } = ops;
        if let Some(entry) = entry {
            insert_entry(txn, entry)?;
        }
        for op in ops {
            Self::insert_rendered_op(txn, op)?;
        }
        Ok(())
    }

    async fn merge_ops_into_cache(&mut self, responses: Vec<WireOps>) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        cache
            .async_commit(|txn| {
                for response in responses {
                    let ops = response.render()?;
                    Self::insert_rendered_ops(txn, ops)?;
                }
                CascadeResult::Ok(())
            })
            .await?;
        Ok(())
    }

    async fn merge_link_ops_into_cache(
        &mut self,
        responses: Vec<WireLinkOps>,
        key: WireLinkKey,
    ) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        cache
            .async_commit(move |txn| {
                for response in responses {
                    let ops = response.render(&key)?;
                    Self::insert_rendered_ops(txn, ops)?;
                }
                CascadeResult::Ok(())
            })
            .await?;
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_element(
        &mut self,
        hash: AnyDhtHash,
        options: NetworkGetOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let results = network
            .get(hash, options.clone())
            .instrument(debug_span!("fetch_element::network_get"))
            .await?;

        self.merge_ops_into_cache(results).await?;
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_links(
        &mut self,
        link_key: WireLinkKey,
        options: GetLinksOptions,
    ) -> CascadeResult<()> {
        let network = ok_or_return!(self.network.as_mut());
        let results = network.get_links(link_key.clone(), options).await?;

        self.merge_link_ops_into_cache(results, link_key.clone())
            .await?;
        Ok(())
    }

    #[instrument(skip(self, options))]
    async fn fetch_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<Vec<AgentActivityResponse<HeaderHash>>> {
        let network = ok_or_return!(self.network.as_mut(), Vec::with_capacity(0));
        Ok(network.get_agent_activity(agent, query, options).await?)
    }

    fn cascading<Q>(&mut self, query: Q) -> CascadeResult<Q::Output>
    where
        Q: Query<Item = Judged<SignedHeaderHashed>>,
    {
        let mut conns = Vec::new();
        let mut txns = Vec::new();
        if let Some(cache) = &mut self.cache {
            conns.push(cache.conn()?);
        }
        if let Some(dht) = &mut self.dht {
            conns.push(dht.conn()?);
        }
        if let Some(authored) = &mut self.authored {
            conns.push(authored.conn()?);
        }
        for conn in &mut conns {
            let txn = conn.transaction().map_err(StateQueryError::from)?;
            txns.push(txn);
        }
        let txns_ref: Vec<_> = txns.iter().collect();
        let results = match &self.scratch {
            Some(scratch) => {
                scratch.apply_and_then(|scratch| query.run(DbScratch::new(&txns_ref, scratch)))?
            }
            None => query.run(Txns::from(&txns_ref[..]))?,
        };
        Ok(results)
    }

    /// Search through the stores and return the first non-none result.
    fn find_map<F, T>(&mut self, mut f: F) -> CascadeResult<Option<T>>
    where
        F: FnMut(&dyn Store) -> CascadeResult<Option<T>>,
    {
        if let Some(cache) = &mut self.cache {
            let mut conn = cache.conn()?;
            let txn = conn.transaction().map_err(StateQueryError::from)?;
            let txn = Txn::from(&txn);
            let r = f(&txn)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        if let Some(dht) = &mut self.dht {
            let mut conn = dht.conn()?;
            let txn = conn.transaction().map_err(StateQueryError::from)?;
            let txn = Txn::from(&txn);
            let r = f(&txn)?;
            if r.is_some() {
                return Ok(r);
            }
        }
        if let Some(authored) = &mut self.authored {
            let mut conn = authored.conn()?;
            let txn = conn.transaction().map_err(StateQueryError::from)?;
            let txn = Txn::from(&txn);
            let r = f(&txn)?;
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

    /// Retrieve [`Entry`] from either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    pub async fn retrieve_entry(
        &mut self,
        hash: EntryHash,
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<EntryHashed>> {
        let private_data = self.private_data.clone();
        let result = self.find_map(|store| {
            Ok(store
                .get_public_or_authored_entry(&hash, private_data.as_ref().map(|a| a.as_ref()))?)
        })?;
        if result.is_some() {
            return Ok(result.map(EntryHashed::from_content_sync));
        }
        options.request_type = holochain_p2p::event::GetRequest::Pending;
        self.fetch_element(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let private_data = self.private_data.clone();
        let result = self.find_map(|store| {
            Ok(store
                .get_public_or_authored_entry(&hash, private_data.as_ref().map(|a| a.as_ref()))?)
        })?;
        Ok(result.map(EntryHashed::from_content_sync))
    }

    /// Retrieve [`SignedHeaderHashed`] from either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    pub async fn retrieve_header(
        &mut self,
        hash: HeaderHash,
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<SignedHeaderHashed>> {
        let result = self.find_map(|store| Ok(store.get_header(&hash)?))?;
        if result.is_some() {
            return Ok(result);
        }
        options.request_type = holochain_p2p::event::GetRequest::Pending;
        self.fetch_element(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let result = self.find_map(|store| Ok(store.get_header(&hash)?))?;
        Ok(result)
    }

    /// Retrieve data from either locally or from an authority.
    /// Data might not have been validated yet by the authority.
    pub async fn retrieve(
        &mut self,
        hash: AnyDhtHash,
        mut options: NetworkGetOptions,
    ) -> CascadeResult<Option<Element>> {
        let private_data = self.private_data.clone();
        let result = self.find_map(|store| {
            Ok(store
                .get_public_or_authored_element(&hash, private_data.as_ref().map(|a| a.as_ref()))?)
        })?;
        if result.is_some() {
            return Ok(result);
        }
        options.request_type = holochain_p2p::event::GetRequest::Pending;
        self.fetch_element(hash.clone(), options).await?;

        let private_data = self.private_data.clone();
        // Check if we have the data now after the network call.
        let result = self.find_map(|store| {
            Ok(store
                .get_public_or_authored_element(&hash, private_data.as_ref().map(|a| a.as_ref()))?)
        })?;
        Ok(result)
    }

    #[instrument(skip(self, options))]
    pub async fn get_entry_details(
        &mut self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<EntryDetails>> {
        let authoring = self.am_i_authoring(&entry_hash.clone().into())?;
        let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
        let query = match self.private_data.clone() {
            Some(author) => {
                GetEntryDetailsQuery::with_private_data_access(entry_hash.clone(), author)
            }
            None => GetEntryDetailsQuery::new(entry_hash.clone()),
        };

        // We don't need metadata and only need the content
        // so if we have it locally then we can avoid the network.
        if let GetStrategy::Content = options.strategy {
            let results = self.cascading(query.clone())?;
            // We got a result so can short circuit.
            if results.is_some() {
                return Ok(results);
            // We didn't get a result so if we are either authoring
            // or the authority there's nothing left to do.
            } else if authoring || authority {
                return Ok(None);
            }
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        // TODO: do we want to put this behind an option, to allow cache-only queries?
        if !(authoring || authority) {
            self.fetch_element(entry_hash.into(), options.into())
                .await?;
        }

        // Check if we have the data now after the network call.
        let results = self.cascading(query)?;
        Ok(results)
    }

    #[instrument(skip(self, options))]
    pub async fn get_header_details(
        &mut self,
        header_hash: HeaderHash,
        options: GetOptions,
    ) -> CascadeResult<Option<ElementDetails>> {
        let authoring = self.am_i_authoring(&header_hash.clone().into())?;
        let authority = self.am_i_an_authority(header_hash.clone().into()).await?;
        let query = match self.private_data.clone() {
            Some(author) => {
                GetElementDetailsQuery::with_private_data_access(header_hash.clone(), author)
            }
            None => GetElementDetailsQuery::new(header_hash.clone()),
        };

        // TODO: we can short circuit if we have any local deletes on a header.
        // Is this bad because we will not go back to the network until our
        // cache is cleared. Could someone create an attack based on this fact?

        // We don't need metadata and only need the content
        // so if we have it locally then we can avoid the network.
        if let GetStrategy::Content = options.strategy {
            let results = self.cascading(query.clone())?;
            // We got a result so can short circuit.
            if results.is_some() {
                return Ok(results);
            // We didn't get a result so if we are either authoring
            // or the authority there's nothing left to do.
            } else if authoring || authority {
                return Ok(None);
            }
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        // TODO: do we want to put this behind an option, to allow cache-only queries?
        if !(authoring || authority) {
            self.fetch_element(header_hash.into(), options.into())
                .await?;
        }

        // Check if we have the data now after the network call.
        let results = self.cascading(query)?;
        Ok(results)
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
        let authoring = self.am_i_authoring(&header_hash.clone().into())?;
        let authority = self.am_i_an_authority(header_hash.clone().into()).await?;
        let query = match self.private_data.clone() {
            Some(author) => {
                GetLiveElementQuery::with_private_data_access(header_hash.clone(), author)
            }
            None => GetLiveElementQuery::new(header_hash.clone()),
        };

        // TODO: we can short circuit if we have any local deletes on a header.
        // Is this bad because we will not go back to the network until our
        // cache is cleared. Could someone create an attack based on this fact?

        // We don't need metadata and only need the content
        // so if we have it locally then we can avoid the network.
        if let GetStrategy::Content = options.strategy {
            let results = self.cascading(query.clone())?;
            // We got a result so can short circuit.
            if results.is_some() {
                return Ok(results);
            // We didn't get a result so if we are either authoring
            // or the authority there's nothing left to do.
            } else if authoring || authority {
                return Ok(None);
            }
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        // TODO: do we want to put this behind an option, to allow cache-only queries?
        if !(authoring || authority) {
            self.fetch_element(header_hash.into(), options.into())
                .await?;
        }

        // Check if we have the data now after the network call.
        let results = self.cascading(query)?;
        Ok(results)
    }

    #[instrument(skip(self, options))]
    /// Returns the oldest live [Element] for this [EntryHash] by getting the
    /// latest available metadata from authorities combined with this agents authored data.
    pub async fn dht_get_entry(
        &mut self,
        entry_hash: EntryHash,
        options: GetOptions,
    ) -> CascadeResult<Option<Element>> {
        let authoring = self.am_i_authoring(&entry_hash.clone().into())?;
        let authority = self.am_i_an_authority(entry_hash.clone().into()).await?;
        let query = match self.private_data.clone() {
            Some(author) => GetLiveEntryQuery::with_private_data_access(entry_hash.clone(), author),
            None => GetLiveEntryQuery::new(entry_hash.clone()),
        };

        // We don't need metadata and only need the content
        // so if we have it locally then we can avoid the network.
        if let GetStrategy::Content = options.strategy {
            let results = self.cascading(query.clone())?;
            // We got a result so can short circuit.
            if results.is_some() {
                return Ok(results);
            // We didn't get a result so if we are either authoring
            // or the authority there's nothing left to do.
            } else if authoring || authority {
                return Ok(None);
            }
        }

        // If we are not in the process of authoring this hash or its
        // authority we need a network call.
        // TODO: do we want to put this behind an option, to allow cache-only queries?
        if !(authoring || authority) {
            self.fetch_element(entry_hash.into(), options.into())
                .await?;
        }

        // Check if we have the data now after the network call.
        let results = self.cascading(query)?;
        Ok(results)
    }

    pub async fn get_concurrent<I: IntoIterator<Item = AnyDhtHash>>(
        &mut self,
        hashes: I,
        options: GetOptions,
    ) -> CascadeResult<Vec<Option<Element>>> {
        use futures::stream::StreamExt;
        use futures::stream::TryStreamExt;
        let iter = hashes.into_iter().map({
            |hash| {
                let options = options.clone();
                let mut cascade = self.clone();
                async move { cascade.dht_get(hash, options).await }
            }
        });
        Ok(futures::stream::iter(iter)
            .buffer_unordered(10)
            .try_collect()
            .await?)
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

    #[instrument(skip(self, options))]
    /// Gets an links from the cas or cache depending on it's metadata
    // The default behavior is to skip deleted or replaced entries.
    pub async fn dht_get_links(
        &mut self,
        key: WireLinkKey,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<Link>> {
        let authority = self.am_i_an_authority(key.base.clone().into()).await?;
        if !authority {
            self.fetch_links(key.clone(), options).await?;
        }
        let query = GetLinksQuery::new(key.base, key.zome_id, key.tag);
        let results = self.cascading(query)?;
        Ok(results)
    }

    #[instrument(skip(self, key, options))]
    /// Return all CreateLink headers
    /// and DeleteLink headers ordered by time.
    pub async fn get_link_details(
        &mut self,
        key: WireLinkKey,
        options: GetLinksOptions,
    ) -> CascadeResult<Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)>> {
        let authority = self.am_i_an_authority(key.base.clone().into()).await?;
        if !authority {
            self.fetch_links(key.clone(), options).await?;
        }
        let query = GetLinkDetailsQuery::new(key.base, key.zome_id, key.tag);
        let results = self.cascading(query)?;
        Ok(results)
    }

    #[instrument(skip(self, agent, query, options))]
    /// Get agent activity from agent activity authorities.
    /// Hashes are requested from the authority and cache for valid chains.
    /// Options:
    /// - include_valid_activity will include the valid chain hashes.
    /// - include_rejected_activity will include the invalid chain hashes.
    /// - include_full_headers will fetch the valid headers in parallel (requires include_valid_activity)
    /// Query:
    /// - include_entries will also fetch the entries in parallel (requires include_full_headers)
    /// - sequence_range will get all the activity in the exclusive range
    /// - header_type and entry_type will filter the activity (requires include_full_headers)
    pub async fn get_agent_activity(
        &mut self,
        agent: AgentPubKey,
        query: ChainQueryFilter,
        options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse<Element>> {
        let status_only = !options.include_rejected_activity && !options.include_valid_activity;
        // TODO: Evaluate if it's ok to **not** go to another authority for agent activity?
        let authority = self.am_i_an_authority(agent.clone().into()).await?;
        let merged_response = if !authority {
            let results = self
                .fetch_agent_activity(agent.clone(), query.clone(), options.clone())
                .await?;
            let merged_response: AgentActivityResponse<HeaderHash> =
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

        // If they don't want the full headers then just return the hashes.
        if !options.include_full_headers {
            return Ok(AgentActivityResponse::hashes_only(merged_response));
        }

        // If they need the full headers then we will do concurrent gets.
        let AgentActivityResponse {
            agent,
            valid_activity,
            rejected_activity,
            status,
            highest_observed,
        } = merged_response;
        let valid_activity = match valid_activity {
            ChainItems::Hashes(hashes) => {
                // If we can't get one of the headers then don't return any.
                // TODO: Is this the correct choice?
                let maybe_chain: Option<Vec<_>> = self
                    .get_concurrent(
                        hashes.into_iter().map(|(_, h)| h.into()),
                        GetOptions::content(),
                    )
                    .await?
                    .into_iter()
                    .collect();
                match maybe_chain {
                    Some(chain) => ChainItems::Full(chain),
                    None => ChainItems::Full(Vec::with_capacity(0)),
                }
            }
            ChainItems::Full(_) => ChainItems::Full(Vec::with_capacity(0)),
            ChainItems::NotRequested => ChainItems::NotRequested,
        };
        let rejected_activity = match rejected_activity {
            ChainItems::Hashes(hashes) => {
                // If we can't get one of the headers then don't return any.
                // TODO: Is this the correct choice?
                let maybe_chain: Option<Vec<_>> = self
                    .get_concurrent(
                        hashes.into_iter().map(|(_, h)| h.into()),
                        GetOptions::content(),
                    )
                    .await?
                    .into_iter()
                    .collect();
                match maybe_chain {
                    Some(chain) => ChainItems::Full(chain),
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

    /// Get the validation package if it is cached without going to the network
    pub fn get_validation_package_local(
        &self,
        _hash: &HeaderHash,
    ) -> CascadeResult<Option<Vec<Element>>> {
        Ok(None)
    }

    pub async fn get_validation_package(
        &mut self,
        _agent: AgentPubKey,
        _header: &HeaderHashed,
    ) -> CascadeResult<Option<ValidationPackage>> {
        Ok(None)
    }

    fn am_i_authoring(&mut self, hash: &AnyDhtHash) -> CascadeResult<bool> {
        let scratch = ok_or_return!(self.scratch.as_ref(), false);
        Ok(scratch.apply_and_then(|scratch| scratch.contains_hash(hash))?)
    }

    async fn am_i_an_authority(&mut self, hash: AnyDhtHash) -> CascadeResult<bool> {
        let network = ok_or_return!(self.network.as_mut(), false);

        Ok(network.authority_for_hash(hash).await?)
    }
}
