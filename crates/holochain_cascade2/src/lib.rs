//! # Cascade
//! ## Retrieve vs Get
//! Get checks CRUD metadata before returning an the data
//! where as retrieve only checks that where the data was found
//! the appropriate validation has been run.

use authority::WireDhtOp;
use authority::WireElementOps;
use authority::WireEntryOps;
use authority::WireOps;
use error::CascadeResult;
use holo_hash::hash_type::AnyDht;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HeaderHash;
use holochain_p2p::actor::GetActivityOptions;
use holochain_p2p::actor::GetLinksOptions;
use holochain_p2p::actor::GetOptions as NetworkGetOptions;
use holochain_sqlite::rusqlite::Transaction;
use holochain_state::insert::update_op_validation_status;
use holochain_state::prelude::*;
use holochain_state::query::element_details::GetElementDetailsQuery;
use holochain_state::query::entry_details::GetEntryDetailsQuery;
use holochain_state::query::live_element::GetLiveElementQuery;
use holochain_state::query::live_entry::GetLiveEntryQuery;
use holochain_state::query::DbScratch;
use holochain_state::query::StateQueryError;
use holochain_state::scratch::Scratch;
use holochain_types::prelude::*;
use insert::insert_entry;
use insert::insert_header;
use insert::insert_op_lite;
use test_utils::HolochainP2pCellT2;
use test_utils::PassThroughNetwork;
use tracing::*;

pub mod authority;
pub mod error;

// FIXME: Make this test_utils feature once we update to
// the real network.
// #[cfg(any(test, feature = "test_utils"))]
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

pub struct Cascade<Network = PassThroughNetwork> {
    vault: Option<EnvRead>,
    cache: Option<EnvWrite>,
    scratch: Option<Scratch>,
    network: Option<Network>,
}

impl<Network> Cascade<Network>
where
    Network: HolochainP2pCellT2 + Clone + 'static + Send,
{
    /// Constructs an empty [Cascade].
    pub fn empty() -> Self {
        Self {
            vault: None,
            network: None,
            cache: None,
            scratch: None,
        }
    }

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
    pub fn with_scratch(self, scratch: Scratch) -> Self {
        Self {
            scratch: Some(scratch),
            ..self
        }
    }

    /// Add the network and cache to the cascade.
    pub fn with_network<N: HolochainP2pCellT2 + Clone>(
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

impl<Network> Cascade<Network>
where
    Network: HolochainP2pCellT2 + Clone + 'static + Send,
{
    fn insert_wire_op(txn: &mut Transaction, wire_op: WireDhtOp) -> CascadeResult<()> {
        let WireDhtOp {
            op_type,
            header,
            signature,
            validation_status,
        } = wire_op;
        let (header, op_hash) = UniqueForm::op_hash(op_type, header)?;
        let header_hashed = HeaderHashed::from_content_sync(header);
        // TODO: Verify signature?
        let header_hashed = SignedHeaderHashed::with_presigned(header_hashed, signature);
        let op_light = DhtOpLight::from_type(
            op_type,
            header_hashed.as_hash().clone(),
            header_hashed.header(),
        )?;

        insert_header(txn, header_hashed);
        insert_op_lite(txn, op_light, op_hash.clone(), false);
        if let Some(status) = validation_status {
            update_op_validation_status(txn, op_hash.clone(), status);
        }
        // We set the integrated to for the cache so it can match the
        // same query as the vault. This can also be used for garbage collection.
        set_when_integrated(txn, op_hash, timestamp::now());
        Ok(())
    }

    fn verify_entry_hash(
        header_entry_hash: Option<&EntryHash>,
        entry_hash: &Option<EntryHash>,
    ) -> bool {
        match (header_entry_hash, entry_hash) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }

    fn merge_entry_ops_into_cache(&mut self, response: WireEntryOps) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        let WireEntryOps {
            creates,
            updates,
            deletes,
            entry,
        } = response;
        cache.conn()?.with_commit(|txn| {
            let mut entry_hash = None;

            if let Some(entry) = entry {
                let entry_hashed = EntryHashed::from_content_sync(entry);
                entry_hash = Some(entry_hashed.as_hash().clone());
                insert_entry(txn, entry_hashed);
            }
            // Do we need to be able to handle creates without an entry.
            // I think we do because we might already have the entry.
            for op in creates {
                if Self::verify_entry_hash(op.header.entry_hash(), &entry_hash) {
                    Self::insert_wire_op(txn, op)?;
                } else {
                    tracing::info!(
                        "Store entry op {:?} from the network entry hash does not match the header",
                        op
                    );
                }
            }
            for op in updates {
                Self::insert_wire_op(txn, op)?;
            }
            for op in deletes {
                Self::insert_wire_op(txn, op)?;
            }
            CascadeResult::Ok(())
        })?;
        Ok(())
    }

    fn merge_element_ops_into_cache(&mut self, response: WireElementOps) -> CascadeResult<()> {
        let cache = ok_or_return!(self.cache.as_mut());
        let WireElementOps {
            header,
            updates,
            deletes,
            entry,
        } = response;
        cache.conn()?.with_commit(|txn| {
            let mut entry_hash = None;

            if let Some(entry) = entry {
                let entry_hashed = EntryHashed::from_content_sync(entry);
                entry_hash = Some(entry_hashed.as_hash().clone());
                insert_entry(txn, entry_hashed);
            }
            if let Some(op) = header {
                if Self::verify_entry_hash(op.header.entry_hash(), &entry_hash) {
                    Self::insert_wire_op(txn, op)?;
                } else {
                    tracing::info!(
                        "Store element op {:?} from the network entry hash does not match the header",
                        op
                    );
                }
            }
            for op in updates {
                Self::insert_wire_op(txn, op)?;
            }
            for op in deletes {
                Self::insert_wire_op(txn, op)?;
            }
            CascadeResult::Ok(())
        })?;
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
            .instrument(debug_span!("fetch_element_via_entry::network_get"))
            .await?;

        for response in results {
            match response {
                WireOps::Entry(response) => self.merge_entry_ops_into_cache(response)?,
                WireOps::Element(response) => self.merge_element_ops_into_cache(response)?,
            }
        }
        Ok(())
    }

    /// Check if this hash has been validated.
    /// Elements can end up in the cache or integrated table because
    /// they were gossiped to you or you authored them.
    /// If you care about the hash you are using being valid in the same
    /// way as if you got it from the StoreElement authority you can     
    /// this function to verify that constraint.
    ///
    /// An example of how this could go wrong is you do a get for a HeaderHash
    /// where you are the authority for the RegisterAgentActivity for this header.
    /// That hash is in your integrated db so you find it but the element has failed
    /// app validation. The header appears valid even though it isn't because as a
    /// RegisterAgentActivity authority you haven't run app validation.
    pub fn valid_header(&self, _hash: &HeaderHash) -> CascadeResult<bool> {
        todo!("I'm guessing we can remove this function")
    }

    /// Same as valid_header but checks for StoreEntry validation
    /// See valid_header for details
    pub fn valid_entry(&self, _hash: &EntryHash) -> CascadeResult<bool> {
        todo!("I'm guessing we can remove this function")
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
        Q: Query<Data = ValStatusOf<SignedHeaderHashed>>,
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
            Some(scratch) => query.run(DbScratch::new(&txns_ref, scratch))?,
            None => query.run(Txns::from(&txns_ref[..]))?,
        };
        Ok(results)
    }

    /// Same as retrieve entry but retrieves many
    /// entries in parallel
    pub async fn retrieve_entries_parallel<'iter, I: IntoIterator<Item = EntryHash>>(
        &mut self,
        _hashes: I,
        _options: NetworkGetOptions,
    ) -> CascadeResult<Vec<Option<EntryHashed>>> {
        todo!()
    }

    /// Same as retrieve_header but retrieves many
    /// elements in parallel
    pub async fn retrieve_headers_parallel<'iter, I: IntoIterator<Item = HeaderHash>>(
        &mut self,
        _hashes: I,
        _options: NetworkGetOptions,
    ) -> CascadeResult<Vec<Option<SignedHeaderHashed>>> {
        todo!()
    }

    /// Same as retrieve but retrieves many
    /// elements in parallel
    pub async fn retrieve_parallel<'iter, I: IntoIterator<Item = HeaderHash>>(
        &mut self,
        _hashes: I,
        _options: NetworkGetOptions,
    ) -> CascadeResult<Vec<Option<Element>>> {
        todo!()
    }

    /// Get the entry from the dht regardless of metadata or validation status.
    /// This call has the opportunity to hit the local cache
    /// and avoid a network call.
    // TODO: This still fetches the full element and metadata.
    // Need to add a fetch_retrieve_entry that only gets data.
    pub async fn retrieve_entry(
        &mut self,
        _hash: EntryHash,
        _options: NetworkGetOptions,
    ) -> CascadeResult<Option<EntryHashed>> {
        todo!()
    }

    /// Get only the header from the dht regardless of metadata or validation status.
    /// Useful for avoiding getting the Entry if you don't need it.
    /// This call has the opportunity to hit the local cache
    /// and avoid a network call.
    // TODO: This still fetches the full element and metadata.
    // Need to add a fetch_retrieve_header that only gets data.
    pub async fn retrieve_header(
        &mut self,
        _hash: HeaderHash,
        _options: NetworkGetOptions,
    ) -> CascadeResult<Option<SignedHeaderHashed>> {
        todo!()
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
        _hash: AnyDhtHash,
        _options: NetworkGetOptions,
    ) -> CascadeResult<Option<Element>> {
        todo!()
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

    #[instrument(skip(self, _key, _options))]
    /// Gets an links from the cas or cache depending on it's metadata
    // The default behavior is to skip deleted or replaced entries.
    // TODO: Implement customization of this behavior with an options/builder struct
    pub async fn dht_get_links<'link>(
        &mut self,
        _key: &'link LinkMetaKey<'link>,
        _options: GetLinksOptions,
    ) -> CascadeResult<Vec<Link>> {
        todo!()
    }

    #[instrument(skip(self, _key, _options))]
    /// Return all CreateLink headers
    /// and DeleteLink headers ordered by time.
    pub async fn get_link_details<'link>(
        &mut self,
        _key: &'link LinkMetaKey<'link>,
        _options: GetLinksOptions,
    ) -> CascadeResult<Vec<(SignedHeaderHashed, Vec<SignedHeaderHashed>)>> {
        todo!()
    }

    // TODO: The whole chain needs to be retrieved so we can
    // check if the headers match the filter but we could store
    // header types / entry types in the activity db to avoid this.
    #[instrument(skip(self, _agent, _query, _options))]
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
        _agent: AgentPubKey,
        _query: ChainQueryFilter,
        _options: GetActivityOptions,
    ) -> CascadeResult<AgentActivityResponse<Element>> {
        todo!()
    }

    /// Get the validation package if it is cached without going to the network
    pub fn get_validation_package_local(
        &self,
        _hash: &HeaderHash,
    ) -> CascadeResult<Option<Vec<Element>>> {
        todo!()
    }

    pub async fn get_validation_package(
        &mut self,
        _agent: AgentPubKey,
        _header: &HeaderHashed,
    ) -> CascadeResult<Option<ValidationPackage>> {
        todo!()
    }

    fn am_i_authoring(&mut self, hash: &AnyDhtHash) -> CascadeResult<bool> {
        let scratch = ok_or_return!(self.scratch.as_ref(), false);
        Ok(scratch.contains_hash(hash)?)
    }

    async fn am_i_an_authority(&mut self, hash: AnyDhtHash) -> CascadeResult<bool> {
        let network = ok_or_return!(self.network.as_mut(), false);
        Ok(network.authority_for_hash(hash).await?)
    }
}
