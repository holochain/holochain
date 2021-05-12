//! # Cascade
//! ## Retrieve vs Get
//! Get checks CRUD metadata before returning an the data
//! where as retrieve only checks that where the data was found
//! the appropriate validation has been run.

use error::CascadeResult;
use holo_hash::hash_type::AnyDht;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_p2p::HolochainP2pCell;
use holochain_p2p::HolochainP2pCellT;
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
pub struct Cascade<Network = HolochainP2pCell> {
    vault: Option<EnvRead>,
    cache: Option<EnvWrite>,
    scratch: Option<SyncScratch>,
    network: Option<Network>,
}

impl<Network> Cascade<Network>
where
    Network: HolochainP2pCellT + Clone + 'static + Send,
{
    /// Add the vault to the cascade.
    pub fn with_vault(self, vault: EnvRead) -> Self {
        Self {
            vault: Some(vault),
            ..self
        }
    }

    /// Add the cache to the cascade.
    // TODO: We do want to be able to use the cache without
    // the network but we always need a cache when we have a
    // network. Perhaps this can be proven at the type level?
    pub fn with_cache(self, cache: EnvWrite) -> Self {
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
    pub fn with_network<N: HolochainP2pCellT + Clone>(
        self,
        network: N,
        cache_env: EnvWrite,
    ) -> Cascade<N> {
        Cascade {
            vault: self.vault,
            scratch: self.scratch,
            cache: Some(cache_env),
            network: Some(network),
        }
    }
}
impl Cascade<HolochainP2pCell> {
    /// Constructs an empty [Cascade].
    pub fn empty() -> Self {
        Self {
            vault: None,
            network: None,
            cache: None,
            scratch: None,
        }
    }

    pub fn from_workspace_network<N: HolochainP2pCellT + Clone>(
        workspace: &HostFnWorkspace,
        network: N,
    ) -> Cascade<N> {
        let HostFnStores {
            vault,
            cache,
            scratch,
        } = workspace.stores();
        Cascade::<N> {
            vault: Some(vault),
            cache: Some(cache),
            scratch: Some(scratch),
            network: Some(network),
        }
    }
    pub fn from_workspace(workspace: &HostFnWorkspace) -> Self {
        let HostFnStores {
            vault,
            cache,
            scratch,
        } = workspace.stores();
        Self {
            vault: Some(vault),
            cache: Some(cache),
            scratch: Some(scratch),
            network: None,
        }
    }
}

impl<Network> Cascade<Network>
where
    Network: HolochainP2pCellT + Clone + 'static + Send,
{
    fn insert_rendered_op(txn: &mut Transaction, op: RenderedOp) -> CascadeResult<()> {
        let RenderedOp {
            op_light,
            op_hash,
            header,
            validation_status,
        } = op;
        let op_order = OpOrder::new(op_light.get_type(), header.header().timestamp());
        insert_header(txn, header)?;
        insert_op_lite(txn, op_light, op_hash.clone(), false, op_order)?;
        if let Some(status) = validation_status {
            set_validation_status(txn, op_hash.clone(), status)?;
        }
        // We set the integrated to for the cache so it can match the
        // same query as the vault. This can also be used for garbage collection.
        set_when_integrated(txn, op_hash, timestamp::now())?;
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

    fn merge_entry_ops_into_cache(&mut self, response: WireEntryOps) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        let ops = response.render()?;
        cache
            .conn()?
            .with_commit(|txn| Self::insert_rendered_ops(txn, ops))?;
        Ok(())
    }

    fn merge_element_ops_into_cache(&mut self, response: WireElementOps) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        let ops = response.render()?;
        cache
            .conn()?
            .with_commit(|txn| Self::insert_rendered_ops(txn, ops))?;
        Ok(())
    }

    fn merge_link_ops_into_cache(
        &mut self,
        response: WireLinkOps,
        key: WireLinkKey,
    ) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        let ops = response.render(key)?;
        cache
            .conn()?
            .with_commit(|txn| Self::insert_rendered_ops(txn, ops))?;
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

        for response in results {
            match response {
                WireOps::Entry(response) => self.merge_entry_ops_into_cache(response)?,
                WireOps::Element(response) => self.merge_element_ops_into_cache(response)?,
            }
        }
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

        for response in results {
            self.merge_link_ops_into_cache(response, link_key.clone())?;
        }
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

    /// Check if we have a valid reason to return an element from the cascade
    /// See valid_header for details
    pub fn valid_element(
        &self,
        _header_hash: &HeaderHash,
        _entry_hash: Option<&EntryHash>,
    ) -> CascadeResult<bool> {
        todo!("I'm guessing we can remove this function")
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
        if let Some(vault) = &mut self.vault {
            conns.push(vault.conn()?);
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
        if let Some(vault) = &mut self.vault {
            let mut conn = vault.conn()?;
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
        let result = self.find_map(|store| Ok(store.get_entry(&hash)?))?;
        if result.is_some() {
            return Ok(result.map(EntryHashed::from_content_sync));
        }
        options.request_type = holochain_p2p::event::GetRequest::Pending;
        self.fetch_element(hash.clone().into(), options).await?;

        // Check if we have the data now after the network call.
        let result = self.find_map(|store| Ok(store.get_entry(&hash)?))?;
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
        let result = self.find_map(|store| Ok(store.get_element(&hash)?))?;
        if result.is_some() {
            return Ok(result);
        }
        options.request_type = holochain_p2p::event::GetRequest::Pending;
        self.fetch_element(hash.clone(), options).await?;

        // Check if we have the data now after the network call.
        let result = self.find_map(|store| Ok(store.get_element(&hash)?))?;
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
        let query = GetEntryDetailsQuery::new(entry_hash.clone());

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

        // If we are not in the process of authoring this hash and we are not the
        // authority we can skip the network call.
        if !authoring && !authority {
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
        let query = GetElementDetailsQuery::new(header_hash.clone());

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

        // If we are not in the process of authoring this hash and we are not the
        // authority we can skip the network call.
        if !authoring && !authority {
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
        let query = GetLiveElementQuery::new(header_hash.clone());

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

        // If we are not in the process of authoring this hash and we are not the
        // authority we can skip the network call.
        if !authoring && !authority {
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
        let query = GetLiveEntryQuery::new(entry_hash.clone());

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

        // If we are not in the process of authoring this hash and we are not the
        // authority we can skip the network call.
        if !authoring && !authority {
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
            match self.vault.clone() {
                Some(vault) => authority::handle_get_agent_activity(
                    vault,
                    agent.clone(),
                    query.clone(),
                    (&options).into(),
                )?,
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
