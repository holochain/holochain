#![allow(clippy::ptr_arg)]
//! # Metadata
//! This module is responsible for generating and storing holochain metadata.
//!
//! [Entry]: holochain_types::Entry

use fallible_iterator::FallibleIterator;
use holo_hash::AgentPubKey;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_lmdb::buffer::KvBufUsed;
use holochain_lmdb::buffer::KvvBufUsed;
use holochain_lmdb::db::CACHE_LINKS_META;
use holochain_lmdb::db::CACHE_STATUS_META;
use holochain_lmdb::db::CACHE_SYSTEM_META;
use holochain_lmdb::db::META_VAULT_LINKS;
use holochain_lmdb::db::META_VAULT_MISC;
use holochain_lmdb::db::META_VAULT_SYS;
use holochain_lmdb::error::DatabaseError;
use holochain_lmdb::error::DatabaseResult;
use holochain_lmdb::fresh_reader;
use holochain_lmdb::prelude::*;
use holochain_serialized_bytes::prelude::*;
use holochain_types::prelude::*;
use holochain_zome_types::HeaderHashed;
use std::collections::HashSet;
use std::fmt::Debug;
use tracing::*;

use activity::*;
pub use keys::*;
pub use sys_meta::*;

#[cfg(any(test, feature = "test_utils"))]
pub use mock::MockMetadataBuf;
#[cfg(any(test, feature = "test_utils"))]
use mockall::mock;

use self::status::DisputedStatus;

mod activity;
#[cfg(test)]
mod chain_test;
mod keys;
#[cfg(test)]
pub mod links_test;
mod status;
mod sys_meta;

#[allow(missing_docs)]
#[cfg(any(test, feature = "test_utils"))]
mod mock;

/// Trait for the [MetadataBuf], needed for mocking
///
/// Unfortunately this cannot be automocked because of the lifetimes required
/// for returning iterators from these trait methods, which automock doesn't support.
pub trait MetadataBufT<P = IntegratedPrefix>
where
    P: PrefixType,
{
    // Links
    /// Get all the links on this base that match the tag
    /// that do not have removes on them
    fn get_live_links<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>;

    /// Get all the links on this base that match the tag regardless of removes
    fn get_links_all<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>;

    /// Add a link
    fn add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()>;

    /// Register a HeaderHash directly on an entry hash.
    /// Also updates the entry dht status.
    /// Useful when you only have hashes and not full types
    fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()>;

    /// Deregister a HeaderHash directly on an entry hash.
    /// Also updates the entry dht status.
    /// Useful when you only have hashes and not full types
    fn deregister_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()>;

    /// Register a value directly on a header hash.
    /// Useful when you only have hashes and not full types
    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);

    /// Register a value directly on a header hash.
    /// Useful when you only have hashes and not full types
    fn deregister_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);

    /// Remove a link
    fn delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()>;

    /// Deregister an add link
    /// Not the same as remove like.
    /// "deregister" removes the data from the metadata store.
    fn deregister_add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()>;

    /// Deregister a remove link
    fn deregister_delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()>;

    /// Registers a [Header::NewEntryHeader] on the referenced [Entry]
    fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Deregister a [Header::NewEntryHeader] on the referenced [Entry]
    fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Registers a rejected [Header::NewEntryHeader] on the referenced [Entry]
    fn register_rejected_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Deregister a rejected [Header::NewEntryHeader] on the referenced [Entry]
    fn deregister_rejected_header(
        &mut self,
        new_entry_header: NewEntryHeader,
    ) -> DatabaseResult<()>;

    /// Registers a [Header] when a StoreElement is processed.
    /// Useful for knowing if we can serve a header from our element vault
    fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()>;

    /// Deregister a [Header] when a StoreElement is processed.
    /// Useful for knowing if we can serve a header from our element vault
    fn deregister_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()>;

    /// Registers a rejected [Header] when a StoreElement is processed.
    /// Useful for knowing if we can serve a header from our element vault
    fn register_rejected_element_header(&mut self, header: &Header) -> DatabaseResult<()>;

    /// Deregister a rejected [Header] when a StoreElement is processed.
    /// Useful for knowing if we can serve a header from our element vault
    fn deregister_rejected_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()>;

    /// Registers a published [Header] on the authoring agent's public key
    fn register_activity(
        &mut self,
        header: &Header,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()>;

    /// Deregister a published [Header] on the authoring agent's public key
    fn deregister_activity(
        &mut self,
        header: &Header,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()>;

    /// Registers a custom validation package on a [HeaderHash]
    fn register_validation_package(
        &mut self,
        hash: &HeaderHash,
        package: impl IntoIterator<Item = HeaderHash>,
    );

    /// Deregister a custom validation package on a [HeaderHash]
    fn deregister_validation_package(&mut self, header: &HeaderHash);

    /// Register a sequence of activity onto an agent key
    fn register_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        sequence: impl IntoIterator<Item = (u32, HeaderHash)>,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()>;

    /// Deregister a sequence of activity onto an agent key
    fn deregister_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        valid_status: ValidationStatus,
    ) -> DatabaseResult<()>;

    /// Registers the agents chain status on the authoring agent's public key
    fn register_activity_status(
        &mut self,
        agent: &AgentPubKey,
        status: ChainStatus,
    ) -> DatabaseResult<()>;

    /// Deregister the agents chain status on the authoring agent's public key
    fn deregister_activity_status(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;

    /// Registers the highest observed sequence number on an agents chain
    fn register_activity_observed(
        &mut self,
        agent: &AgentPubKey,
        observed: HighestObserved,
    ) -> DatabaseResult<()>;

    /// Deregister the highest observed sequence number on an agents chain
    fn deregister_activity_observed(&mut self, agent: &AgentPubKey) -> DatabaseResult<()>;

    /// Registers a [Header::Update] on the referenced [Header] or [Entry]
    fn register_update(&mut self, update: header::Update) -> DatabaseResult<()>;

    /// Deregister a [Header::Update] on the referenced [Header] or [Entry]
    fn deregister_update(&mut self, update: header::Update) -> DatabaseResult<()>;

    /// Registers a [Header::Delete] on the Header of an Entry
    fn register_delete(&mut self, delete: header::Delete) -> DatabaseResult<()>;

    /// Deregister a [Header::Delete] on the Header of an Entry
    fn deregister_delete(&mut self, delete: header::Delete) -> DatabaseResult<()>;

    /// Registers a ValidationStatus on a Header hash
    fn register_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus);

    /// Deregister a ValidationStatus on a Header hash
    fn deregister_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus);

    /// Returns all the valid [HeaderHash]es of headers that created this [Entry]
    fn get_headers<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the rejected [HeaderHash]es of headers that created this [Entry]
    fn get_rejected_headers<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the valid and rejected [HeaderHash]es of headers that created this [Entry]
    fn get_all_headers<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Get chain items on an agents source chain.
    /// This is how we query for RegisterAgentActivity items.
    ///
    /// The relationship is a key (that can be partially matched)
    /// as [AgentPubKey] then "header sequence index" then HeaderHash.
    ///
    /// There can be multiple headers at a sequence number.
    /// This means there's a fork in the chain.
    /// We store the data as proof.
    fn get_activity<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        key: ChainItemKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Same as get activity but includes the sequence number in the iterator value
    fn get_activity_sequence<'r, R: Readable>(
        &'r self,
        r: &'r R,
        key: ChainItemKey,
    ) -> DatabaseResult<
        Box<dyn FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError> + '_>,
    >;

    /// Get a custom validation package on this header hash
    fn get_validation_package<'r, R: Readable>(
        &'r self,
        r: &'r R,
        hash: &HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>;

    /// Get the current status of this agents chain
    fn get_activity_status(&self, agent: &AgentPubKey) -> DatabaseResult<Option<ChainStatus>>;

    /// Get the current highest observed header on this agents chain
    fn get_activity_observed(&self, agent: &AgentPubKey)
        -> DatabaseResult<Option<HighestObserved>>;

    /// Returns all the hashes of [Update] headers registered on an [Entry]
    fn get_updates<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [Delete] headers registered on a Header
    fn get_deletes_on_header<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [Delete] headers registered on an Entry's header
    fn get_deletes_on_entry<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns the current [EntryDhtStatus] of an [Entry]
    fn get_dht_status<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<EntryDhtStatus>;

    /// Returns the current set of [ValidationStatus] for a [Header].
    /// A set of disputed status is returned.
    /// If the set only contains one entry there is no dispute.
    fn get_validation_status<'r, R: Readable>(
        &'r self,
        r: &'r R,
        header_hash: &HeaderHash,
    ) -> DatabaseResult<DisputedStatus>;

    /// Finds the redirect path and returns the final [Entry]
    fn get_canonical_entry_hash(&self, entry_hash: EntryHash) -> DatabaseResult<EntryHash>;

    /// Finds the redirect path and returns the final [Header]
    fn get_canonical_header_hash(&self, header_hash: HeaderHash) -> DatabaseResult<HeaderHash>;

    /// Returns all the link remove headers attached to a link add header
    fn get_link_removes_on_link_add<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Finds if there is a valid or rejected StoreElement for this header
    fn has_any_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;

    /// Finds if there is a valid StoreElement for this header
    fn has_valid_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;

    /// Finds if there is a rejected StoreElement for this header
    fn has_rejected_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool>;

    /// Finds if there is a StoreEntry for this header
    fn has_registered_store_entry(
        &self,
        entry_hash: &EntryHash,
        header_hash: &HeaderHash,
    ) -> DatabaseResult<bool>;

    /// Finds if there is a StoreEntry for this entry
    fn has_any_registered_store_entry(&self, hash: &EntryHash) -> DatabaseResult<bool>;

    /// Get the environment for creating readers
    fn env(&self) -> &EnvironmentRead;
}

/// Updates and answers queries for the links and system meta databases
pub struct MetadataBuf<P = IntegratedPrefix>
where
    P: PrefixType,
{
    system_meta: KvvBufUsed<PrefixBytesKey<P>, SysMetaVal>,
    links_meta: KvBufUsed<PrefixBytesKey<P>, LinkMetaVal>,
    misc_meta: KvBufUsed<PrefixBytesKey<P>, MiscMetaValue>,
    env: EnvironmentRead,
}

impl MetadataBuf<IntegratedPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the IntegratedPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn vault(env: EnvironmentRead) -> DatabaseResult<Self> {
        Self::new_vault(env)
    }

    /// Create a [MetadataBuf] with the cache databases
    pub fn cache(env: EnvironmentRead) -> DatabaseResult<Self> {
        let system_meta = env.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = env.get_db(&*CACHE_LINKS_META)?;
        let misc_meta = env.get_db(&*CACHE_STATUS_META)?;
        Self::new(env, system_meta, links_meta, misc_meta)
    }
}

impl MetadataBuf<PendingPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the PendingPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn pending(env: EnvironmentRead) -> DatabaseResult<Self> {
        Self::new_vault(env)
    }
}

impl MetadataBuf<RejectedPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the RejectedPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn rejected(env: EnvironmentRead) -> DatabaseResult<Self> {
        Self::new_vault(env)
    }
}

impl MetadataBuf<AuthoredPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the AuthoredPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn authored(env: EnvironmentRead) -> DatabaseResult<Self> {
        Self::new_vault(env)
    }
}

impl<P> MetadataBuf<P>
where
    P: PrefixType,
{
    pub(crate) fn new(
        env: EnvironmentRead,
        system_meta: MultiStore,
        links_meta: SingleStore,
        misc_meta: SingleStore,
    ) -> DatabaseResult<Self> {
        Ok(Self {
            system_meta: KvvBufUsed::new(system_meta),
            links_meta: KvBufUsed::new(links_meta),
            misc_meta: KvBufUsed::new(misc_meta),
            env,
        })
    }

    fn new_vault(env: EnvironmentRead) -> DatabaseResult<Self> {
        let system_meta = env.get_db(&*META_VAULT_SYS)?;
        let links_meta = env.get_db(&*META_VAULT_LINKS)?;
        let misc_meta = env.get_db(&*META_VAULT_MISC)?;
        Self::new(env, system_meta, links_meta, misc_meta)
    }

    fn register_header_on_basis<K, H>(&mut self, key: K, header: H) -> DatabaseResult<()>
    where
        H: Into<EntryHeader>,
        K: Into<SysMetaKey>,
    {
        let sys_val = match header.into() {
            h @ EntryHeader::NewEntry(_) => SysMetaVal::NewEntry(h.into_hash()?),
            h @ EntryHeader::Update(_) => SysMetaVal::Update(h.into_hash()?),
            h @ EntryHeader::Delete(_) => SysMetaVal::Delete(h.into_hash()?),
        };
        let key: SysMetaKey = key.into();
        self.system_meta.insert(PrefixBytesKey::new(key), sys_val);
        Ok(())
    }

    fn deregister_header_on_basis<K, H>(&mut self, key: K, header: H) -> DatabaseResult<()>
    where
        H: Into<EntryHeader>,
        K: Into<SysMetaKey>,
    {
        let sys_val = match header.into() {
            h @ EntryHeader::NewEntry(_) => SysMetaVal::NewEntry(h.into_hash()?),
            h @ EntryHeader::Update(_) => SysMetaVal::Update(h.into_hash()?),
            h @ EntryHeader::Delete(_) => SysMetaVal::Delete(h.into_hash()?),
        };
        let key: SysMetaKey = key.into();
        self.system_meta.delete(PrefixBytesKey::new(key), sys_val);
        Ok(())
    }

    #[instrument(skip(self))]
    fn update_entry_dht_status(&mut self, basis: EntryHash) -> DatabaseResult<()> {
        let status = fresh_reader!(self.env, |r| self.get_headers(&r, basis.clone())?.find_map(
            |header| {
                if self
                    .get_deletes_on_header(&r, header.header_hash)?
                    .next()?
                    .is_none()
                {
                    trace!("found live header");
                    Ok(Some(EntryDhtStatus::Live))
                } else {
                    trace!("found dead header");
                    Ok(None)
                }
            }
        ))?
        // No evidence of life found so entry is marked dead
        .unwrap_or(EntryDhtStatus::Dead);
        self.misc_meta.put(
            MiscMetaKey::entry_status(&basis).into(),
            MiscMetaValue::EntryStatus(status),
        )
    }

    /// If there are any rejected or abandoned activity
    /// return the earliest problem.
    fn check_for_invalid_status<R: Readable>(
        &self,
        agent: AgentPubKey,
        reader: &R,
    ) -> DatabaseResult<ChainStatus> {
        let rejected_key = ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Rejected);
        let abandoned_key = ChainItemKey::AgentStatus(agent, ValidationStatus::Abandoned);
        let rejected_hash = self.get_activity_sequence(reader, rejected_key)?.next()?;
        let abandoned_hash = self.get_activity_sequence(reader, abandoned_key)?.next()?;
        match (rejected_hash, abandoned_hash) {
            (None, None) => Ok(ChainStatus::Empty),
            (None, Some((header_seq, hash))) => {
                Ok(ChainStatus::Invalid(ChainHead { header_seq, hash }))
            }
            (Some((header_seq, hash)), None) => {
                Ok(ChainStatus::Invalid(ChainHead { header_seq, hash }))
            }
            (Some(a), Some(b)) => match a.0.cmp(&b.0) {
                std::cmp::Ordering::Equal => Ok(ChainStatus::Forked(ChainFork {
                    fork_seq: a.0,
                    first_header: a.1,
                    second_header: b.1,
                })),
                std::cmp::Ordering::Less => Ok(ChainStatus::Invalid(ChainHead {
                    header_seq: a.0,
                    hash: a.1,
                })),
                std::cmp::Ordering::Greater => Ok(ChainStatus::Invalid(ChainHead {
                    header_seq: b.0,
                    hash: b.1,
                })),
            },
        }
    }

    /// Check the valid activity sequence is complete and
    /// doesn't have any forks or return the first fork.
    fn calculate_activity_status(
        &self,
        mut activity: impl FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError>,
    ) -> DatabaseResult<Option<ChainStatus>> {
        let mut last = None;
        let mut chain_complete = true;
        while let Some((header_seq, hash)) = activity.next()? {
            if let Some(ChainHead {
                header_seq: last_seq,
                hash: last_hash,
            }) = last
            {
                if last_seq == header_seq {
                    // Chain is forked
                    return Ok(Some(ChainStatus::Forked(ChainFork {
                        fork_seq: last_seq,
                        first_header: last_hash,
                        second_header: hash,
                    })));
                }
                if header_seq != last_seq + 1 {
                    // Chain broken but still check for forks
                    chain_complete = false;
                }
            }
            last = Some(ChainHead { header_seq, hash });
        }
        if chain_complete {
            return Ok(last.map(ChainStatus::Valid));
        }
        DatabaseResult::Ok(None)
    }

    /// Check the activity chain for forks and gaps.
    /// If there is a fork record a forked chain status.
    /// Otherwise if there are no gaps then record a valid chain.
    fn update_activity_status(&mut self, agent: &AgentPubKey) -> DatabaseResult<()> {
        let key = ChainItemKey::AgentStatus(agent.clone(), ValidationStatus::Valid);
        let status = fresh_reader!(self.env, |r| {
            let invalid_activity_status = self.check_for_invalid_status(agent.clone(), &r)?;
            match invalid_activity_status {
                // No invalid data so check entire valid activity
                ChainStatus::Empty => {
                    let iter = self.get_activity_sequence(&r, key)?;
                    self.calculate_activity_status(iter)
                }
                // Invalid data found so check for earlier problems
                // up to the found problem sequence number
                ChainStatus::Invalid(ChainHead {
                    header_seq: seq, ..
                })
                | ChainStatus::Forked(ChainFork { fork_seq: seq, .. }) => {
                    let iter = self
                        .get_activity_sequence(&r, key)?
                        .take_while(|(s, _)| Ok(*s < seq));
                    self.calculate_activity_status(iter)
                }
                _ => unreachable!(),
            }
        })?;
        if let Some(status) = status {
            self.register_activity_status(agent, status)?;
        }
        Ok(())
    }

    #[cfg(any(test, feature = "test_utils"))]
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.links_meta.clear_all(writer)?;
        self.system_meta.clear_all(writer)
    }
}

impl<P> MetadataBufT<P> for MetadataBuf<P>
where
    P: PrefixType,
{
    fn get_live_links<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>
    {
        Ok(Box::new(
            self.links_meta
                .iter_all_key_matches(r, key.into())?
                .filter_map(move |(_, link)| {
                    // Check if link has been removed
                    match self
                        .get_link_removes_on_link_add(r, link.link_add_hash.clone())?
                        .next()?
                    {
                        Some(_) => Ok(None),
                        None => Ok(Some(link)),
                    }
                }),
        ))
    }

    fn get_links_all<'r, 'k, R: Readable>(
        &'r self,
        r: &'r R,
        key: &'k LinkMetaKey<'k>,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = LinkMetaVal, Error = DatabaseError> + 'r>>
    {
        Ok(Box::new(
            self.links_meta
                .iter_all_key_matches(r, key.into())?
                .map(|(_, v)| Ok(v)),
        ))
    }

    fn add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()> {
        // Register the add link onto the base
        let link_add_hash =
            HeaderHashed::from_content_sync(Header::CreateLink(link_add.clone())).into_hash();

        // Put the link add to the links table
        let key = LinkMetaKey::from((&link_add, &link_add_hash));

        self.links_meta.put(
            key.into(),
            LinkMetaVal {
                link_add_hash,
                target: link_add.target_address,
                timestamp: link_add.timestamp.into(),
                zome_id: link_add.zome_id,
                tag: link_add.tag,
            },
        )
    }

    fn deregister_add_link(&mut self, link_add: CreateLink) -> DatabaseResult<()> {
        let link_add_hash = HeaderHash::with_data_sync(&Header::CreateLink(link_add.clone()));
        let key = LinkMetaKey::from((&link_add, &link_add_hash));
        self.links_meta.delete(key.into())
    }

    fn delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()> {
        let link_add_address = link_remove.link_add_address.clone();
        // Register the link remove address to the link add address
        let link_remove = HeaderHashed::from_content_sync(Header::DeleteLink(link_remove));
        let sys_val = SysMetaVal::DeleteLink(link_remove.into());
        self.system_meta
            .insert(SysMetaKey::from(link_add_address).into(), sys_val);
        Ok(())
    }

    fn deregister_delete_link(&mut self, link_remove: DeleteLink) -> DatabaseResult<()> {
        let link_add_address = link_remove.link_add_address.clone();
        // Register the link remove address to the link add address
        let link_remove = HeaderHashed::from_content_sync(Header::DeleteLink(link_remove));
        let sys_val = SysMetaVal::DeleteLink(link_remove.into());
        self.system_meta
            .delete(SysMetaKey::from(link_add_address).into(), sys_val);
        Ok(())
    }

    fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()> {
        self.system_meta
            .insert(SysMetaKey::from(entry_hash.clone()).into(), value);
        self.update_entry_dht_status(entry_hash)
    }

    fn deregister_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()> {
        self.system_meta
            .delete(SysMetaKey::from(entry_hash.clone()).into(), value);
        self.update_entry_dht_status(entry_hash)
    }

    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.system_meta
            .insert(SysMetaKey::from(header_hash).into(), value);
    }

    fn deregister_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.system_meta
            .delete(SysMetaKey::from(header_hash).into(), value);
    }

    fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        self.register_header_on_basis(basis.clone(), new_entry_header)?;
        self.update_entry_dht_status(basis)?;
        Ok(())
    }

    fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        self.deregister_header_on_basis(basis.clone(), new_entry_header)?;
        self.update_entry_dht_status(basis)?;
        Ok(())
    }

    fn register_rejected_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        let header: Header = new_entry_header.into();
        let header = HeaderHashed::from_content_sync(header);
        let value = SysMetaVal::RejectedNewEntry(header.into());
        self.register_raw_on_entry(basis, value)?;
        Ok(())
    }

    fn deregister_rejected_header(
        &mut self,
        new_entry_header: NewEntryHeader,
    ) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        let header: Header = new_entry_header.into();
        let header = HeaderHashed::from_content_sync(header);
        let value = SysMetaVal::RejectedNewEntry(header.into());
        self.deregister_raw_on_entry(basis, value)?;
        Ok(())
    }

    fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()> {
        self.misc_meta.put(
            MiscMetaKey::store_element(&HeaderHash::with_data_sync(header)).into(),
            MiscMetaValue::new_store_element(),
        )
    }

    fn deregister_element_header(&mut self, hash: HeaderHash) -> DatabaseResult<()> {
        self.misc_meta
            .delete(MiscMetaKey::store_element(&hash).into())
    }

    fn register_rejected_element_header(&mut self, header: &Header) -> DatabaseResult<()> {
        self.misc_meta.put(
            MiscMetaKey::rejected_store_element(&HeaderHash::with_data_sync(header)).into(),
            MiscMetaValue::new_store_element(),
        )
    }

    fn deregister_rejected_element_header(&mut self, hash: HeaderHash) -> DatabaseResult<()> {
        self.misc_meta
            .delete(MiscMetaKey::rejected_store_element(&hash).into())
    }

    fn register_update(&mut self, update: header::Update) -> DatabaseResult<()> {
        let header_hash = update.original_header_address.clone();
        let entry_hash = update.original_entry_address.clone();
        self.register_header_on_basis(header_hash, update.clone())?;
        self.register_header_on_basis(entry_hash, update)
    }

    fn deregister_update(&mut self, update: header::Update) -> DatabaseResult<()> {
        let header_hash = update.original_header_address.clone();
        let entry_hash = update.original_entry_address.clone();
        self.deregister_header_on_basis(header_hash, update.clone())?;
        self.deregister_header_on_basis(entry_hash, update)
    }

    fn register_delete(&mut self, delete: header::Delete) -> DatabaseResult<()> {
        let remove = delete.deletes_address.to_owned();
        let entry_hash = delete.deletes_entry_address.clone();
        self.register_header_on_basis(remove, delete.clone())?;
        self.register_header_on_basis(entry_hash.clone(), delete)?;
        self.update_entry_dht_status(entry_hash)
    }

    fn deregister_delete(&mut self, delete: header::Delete) -> DatabaseResult<()> {
        let remove = delete.deletes_address.to_owned();
        let entry_hash = delete.deletes_entry_address.clone();
        self.deregister_header_on_basis(remove, delete.clone())?;
        self.deregister_header_on_basis(entry_hash.clone(), delete)?;
        self.update_entry_dht_status(entry_hash)
    }

    fn register_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus) {
        self.register_raw_on_header(hash, SysMetaVal::ValidationStatus(status))
    }

    fn deregister_validation_status(&mut self, hash: HeaderHash, status: ValidationStatus) {
        self.deregister_raw_on_header(hash, SysMetaVal::ValidationStatus(status))
    }

    fn register_activity(
        &mut self,
        header: &Header,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        let key = ChainItemKey::new(header, validation_status);
        let key = MiscMetaKey::chain_item(&key).into();
        let value = MiscMetaValue::ChainItem(header.timestamp().clone().into());
        self.misc_meta.put(key, value)?;
        self.update_activity_status(header.author())
    }

    fn deregister_activity(
        &mut self,
        header: &Header,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        let key = ChainItemKey::new(header, validation_status);
        self.misc_meta
            .delete(MiscMetaKey::chain_item(&key).into())?;
        self.update_activity_status(header.author())
    }

    fn register_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        sequence: impl IntoIterator<Item = (u32, HeaderHash)>,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        for (seq, hash) in sequence {
            let key = ChainItemKey::Full(agent.clone(), validation_status, seq, hash);
            let key = MiscMetaKey::chain_item(&key).into();
            // TODO: Remove timestamp value as headers are already ordered
            let value = MiscMetaValue::ChainItem(Timestamp::now());
            self.misc_meta.put(key, value)?;
        }
        self.update_activity_status(agent)
    }

    fn deregister_activity_sequence(
        &mut self,
        agent: &AgentPubKey,
        validation_status: ValidationStatus,
    ) -> DatabaseResult<()> {
        let key = ChainItemKey::AgentStatus(agent.clone(), validation_status);
        let sequence: Vec<_> = fresh_reader!(self.env, |r| {
            self.get_activity_sequence(&r, key)?.collect()
        })?;
        for (seq, hash) in sequence {
            let k = ChainItemKey::Full(agent.clone(), validation_status, seq, hash);
            let k = MiscMetaKey::chain_item(&k).into();
            self.misc_meta.delete(k)?;
        }
        self.update_activity_status(agent)
    }

    fn register_validation_package(
        &mut self,
        hash: &HeaderHash,
        package: impl IntoIterator<Item = HeaderHash>,
    ) {
        let key: SysMetaKey = hash.clone().into();
        for hash in package {
            self.system_meta.insert(
                PrefixBytesKey::new(key.clone()),
                SysMetaVal::CustomPackage(hash),
            );
        }
    }

    fn deregister_validation_package(&mut self, hash: &HeaderHash) {
        let key: SysMetaKey = hash.clone().into();
        self.system_meta.delete_all(PrefixBytesKey::new(key));
    }

    fn register_activity_status(
        &mut self,
        agent: &AgentPubKey,
        status: ChainStatus,
    ) -> DatabaseResult<()> {
        let new_status = match self.get_activity_status(agent)? {
            Some(prev_status) => add_chain_status(prev_status, status),
            None => Some(status),
        };
        if let Some(s) = new_status {
            let key = MiscMetaKey::chain_status(&agent).into();
            let value = MiscMetaValue::ChainStatus(s);
            self.misc_meta.put(key, value)?;
        }
        Ok(())
    }

    fn deregister_activity_status(&mut self, agent: &AgentPubKey) -> DatabaseResult<()> {
        self.misc_meta
            .delete(MiscMetaKey::chain_status(&agent).into())
    }

    fn register_activity_observed(
        &mut self,
        agent: &AgentPubKey,
        observed: HighestObserved,
    ) -> DatabaseResult<()> {
        if let Some(mut prev_observed) = self.get_activity_observed(agent)? {
            if prev_observed.header_seq > observed.header_seq {
                // If the previous is more recent then don't overwrite
            } else if prev_observed.header_seq == observed.header_seq
                && prev_observed.hash != observed.hash
            {
                // If the observed are the same sequence
                // Combine the hashes and overwrite
                let diff = observed
                    .hash
                    .into_iter()
                    .filter(|h| prev_observed.hash.contains(h))
                    .collect::<Vec<_>>();
                prev_observed.hash.extend(diff);

                let key = MiscMetaKey::chain_observed(&agent).into();
                let value = MiscMetaValue::ChainObserved(prev_observed);
                self.misc_meta.put(key, value)?;
            } else {
                let key = MiscMetaKey::chain_observed(&agent).into();
                let value = MiscMetaValue::ChainObserved(observed);
                self.misc_meta.put(key, value)?;
            }
        } else {
            let key = MiscMetaKey::chain_observed(&agent).into();
            let value = MiscMetaValue::ChainObserved(observed);
            self.misc_meta.put(key, value)?;
        }
        Ok(())
    }

    fn deregister_activity_observed(&mut self, agent: &AgentPubKey) -> DatabaseResult<()> {
        self.misc_meta
            .delete(MiscMetaKey::chain_observed(&agent).into())
    }

    fn get_headers<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(entry_hash).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::NewEntry(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_all_headers<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(entry_hash).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::NewEntry(h) => Some(h),
                    SysMetaVal::RejectedNewEntry(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_rejected_headers<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(entry_hash).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::RejectedNewEntry(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_updates<'r, R: Readable>(
        &'r self,
        r: &'r R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(self.system_meta.get(r, &hash.into())?).filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Update(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_deletes_on_header<'r, R: Readable>(
        &'r self,
        r: &'r R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(new_entry_header).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Delete(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_deletes_on_entry<'r, R: Readable>(
        &'r self,
        r: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(entry_hash).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Delete(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_activity<'r, R: Readable>(
        &'r self,
        r: &'r R,
        key: ChainItemKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        let k = MiscMetaKey::chain_item(&key).into();
        Ok(Box::new(self.misc_meta.iter_all_key_matches(r, k)?.map(
            |(k, v)| {
                let k: MiscMetaKey<ChainItemPrefix> =
                    PrefixBytesKey::<P>::from_key_bytes_or_friendly_panic(k).into();
                let header_hash = ChainItemKey::from(k).into();
                let timestamp = MiscMetaValue::chain_item(v);
                let r = TimedHeaderHash {
                    timestamp,
                    header_hash,
                };
                Ok(r)
            },
        )))
    }

    fn get_activity_sequence<'r, R: Readable>(
        &'r self,
        r: &'r R,
        key: ChainItemKey,
    ) -> DatabaseResult<
        Box<dyn FallibleIterator<Item = (u32, HeaderHash), Error = DatabaseError> + '_>,
    > {
        let k = MiscMetaKey::chain_item(&key).into();
        Ok(Box::new(self.misc_meta.iter_all_key_matches(r, k)?.map(
            |(k, _)| {
                let k: MiscMetaKey<ChainItemPrefix> =
                    PrefixBytesKey::<P>::from_key_bytes_or_friendly_panic(k).into();
                let key = ChainItemKey::from(k);
                let sequence = (&key).into();
                let header_hash = key.into();
                Ok((sequence, header_hash))
            },
        )))
    }

    fn get_validation_package<'r, R: Readable>(
        &'r self,
        r: &'r R,
        hash: &HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = HeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(hash.clone()).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::CustomPackage(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn get_activity_status(&self, agent: &AgentPubKey) -> DatabaseResult<Option<ChainStatus>> {
        let key = MiscMetaKey::chain_status(&agent).into();
        Ok(fresh_reader!(self.env, |r| self.misc_meta.get(&r, &key))?
            .map(MiscMetaValue::chain_status))
    }

    fn get_activity_observed(
        &self,
        agent: &AgentPubKey,
    ) -> DatabaseResult<Option<HighestObserved>> {
        let key = MiscMetaKey::chain_observed(&agent).into();
        Ok(fresh_reader!(self.env, |r| self.misc_meta.get(&r, &key))?
            .map(MiscMetaValue::chain_observed))
    }

    // TODO: For now this is only checking for deletes
    // Once the validation is finished this should check for that as well
    fn get_dht_status<'r, R: Readable>(
        &self,
        r: &'r R,
        entry_hash: &EntryHash,
    ) -> DatabaseResult<EntryDhtStatus> {
        Ok(self
            .misc_meta
            .get(r, &MiscMetaKey::entry_status(entry_hash).into())?
            .map(MiscMetaValue::entry_status)
            .unwrap_or(EntryDhtStatus::Dead))
    }

    fn get_validation_status<'r, R: Readable>(
        &'r self,
        r: &'r R,
        hash: &HeaderHash,
    ) -> DatabaseResult<DisputedStatus> {
        Ok(fallible_iterator::convert(
            self.system_meta
                .get(r, &SysMetaKey::from(hash.clone()).into())?,
        )
        .filter_map(|h| {
            Ok(match h {
                SysMetaVal::ValidationStatus(s) => Some(s),
                _ => None,
            })
        })
        .collect::<HashSet<_>>()?
        .into())
    }

    fn get_canonical_entry_hash(&self, _entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        todo!("Cannot implement until redirects are implemented")
    }

    fn get_canonical_header_hash(&self, _header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        todo!("Cannot implement until redirects are implemented")
    }

    fn get_link_removes_on_link_add<'r, R: Readable>(
        &'r self,
        r: &'r R,
        link_add: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(link_add).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::DeleteLink(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    fn has_any_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| DatabaseResult::Ok(
            self.misc_meta
                .contains(&r, &MiscMetaKey::store_element(hash).into())?
                || self
                    .misc_meta
                    .contains(&r, &MiscMetaKey::rejected_store_element(hash).into())?
        ))
    }

    fn has_valid_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| self
            .misc_meta
            .contains(&r, &MiscMetaKey::store_element(hash).into()))
    }

    fn has_rejected_registered_store_element(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| self
            .misc_meta
            .contains(&r, &MiscMetaKey::rejected_store_element(hash).into()))
    }

    fn has_registered_store_entry(
        &self,
        entry_hash: &EntryHash,
        header_hash: &HeaderHash,
    ) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| self
            .get_headers(&r, entry_hash.clone())?
            .any(|h| Ok(h.header_hash == *header_hash)))
    }

    fn has_any_registered_store_entry(&self, hash: &EntryHash) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| Ok(self
            .get_headers(&r, hash.clone())?
            .next()?
            .is_some()))
    }

    fn env(&self) -> &EnvironmentRead {
        &self.env
    }
}

impl<P: PrefixType> BufferedStore for MetadataBuf<P> {
    type Error = DatabaseError;

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn_ref(writer)?;
        self.links_meta.flush_to_txn_ref(writer)?;
        self.misc_meta.flush_to_txn_ref(writer)?;
        Ok(())
    }
}
/// Create an Metadata with a clone of the scratch
/// from another MetadataBuf
impl<P> From<&MetadataBuf<P>> for MetadataBuf<P>
where
    P: PrefixType,
{
    fn from(other: &MetadataBuf<P>) -> Self {
        Self {
            system_meta: (&other.system_meta).into(),
            links_meta: (&other.links_meta).into(),
            misc_meta: (&other.misc_meta).into(),
            env: other.env.clone(),
        }
    }
}
