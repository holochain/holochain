#![allow(clippy::ptr_arg)]
//! # Metadata
//! This module is responsible for generating and storing holochain metadata.
//!
//! [Entry]: holochain_types::Entry

use fallible_iterator::FallibleIterator;
use holo_hash::HasHash;
use holo_hash::{AgentPubKey, AnyDhtHash, EntryHash, HeaderHash};
use holochain_serialized_bytes::prelude::*;
use holochain_state::{
    buffer::{KvBufUsed, KvvBufUsed},
    db::{
        CACHE_LINKS_META, CACHE_STATUS_META, CACHE_SYSTEM_META, META_VAULT_LINKS, META_VAULT_MISC,
        META_VAULT_SYS,
    },
    error::{DatabaseError, DatabaseResult},
    fresh_reader,
    prelude::*,
};
use holochain_types::metadata::{EntryDhtStatus, TimedHeaderHash};
use holochain_types::{header::NewEntryHeader, link::WireLinkMetaKey};
use holochain_types::{HeaderHashed, Timestamp};
use holochain_zome_types::header::{self, LinkAdd, LinkRemove, ZomeId};
use holochain_zome_types::{link::LinkTag, Header};
use std::fmt::Debug;
use tracing::*;

pub use keys::*;
pub use sys_meta::*;

#[cfg(test)]
pub use mock::MockMetadataBuf;
#[cfg(test)]
use mockall::mock;

mod keys;
#[cfg(test)]
pub mod links_test;
mod sys_meta;

#[allow(missing_docs)]
#[cfg(test)]
mod mock;

/// Trait for the [MetadataBuf]
/// Needed for mocking
#[async_trait::async_trait]
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
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;

    /// Register a HeaderHash directly on an entry hash.
    /// Also updates the entry dht status.
    /// Useful when you only have hashes and not full types
    fn register_raw_on_entry(
        &mut self,
        entry_hash: EntryHash,
        value: SysMetaVal,
    ) -> DatabaseResult<()>;

    /// Register a HeaderHash directly on a header hash.
    /// Useful when you only have hashes and not full types
    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal);

    /// Remove a link
    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()>;

    /// Deregister an add link
    /// Not the same as remove like.
    /// "deregister" removes the data from the metadata store.
    async fn deregister_add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()>;

    /// Deregister a remove link
    async fn deregister_remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()>;

    /// Registers a [Header::NewEntryHeader] on the referenced [Entry]
    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Deregister a [Header::NewEntryHeader] on the referenced [Entry]
    async fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()>;

    /// Registers a [Header] when a StoreElement is processed.
    /// Useful for knowing if we can serve a header from our element vault
    async fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()>;

    /// Deregister a [Header] when a StoreElement is processed.
    /// Useful for knowing if we can serve a header from our element vault
    async fn deregister_element_header(&mut self, header: HeaderHash) -> DatabaseResult<()>;

    /// Registers a published [Header] on the authoring agent's public key
    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()>;

    /// Deregister a published [Header] on the authoring agent's public key
    async fn deregister_activity(&mut self, header: Header) -> DatabaseResult<()>;

    /// Registers a [Header::EntryUpdate] on the referenced [Header] or [Entry]
    async fn register_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()>;

    /// Deregister a [Header::EntryUpdate] on the referenced [Header] or [Entry]
    async fn deregister_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()>;

    /// Registers a [Header::ElementDelete] on the Header of an Entry
    async fn register_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()>;

    /// Deregister a [Header::ElementDelete] on the Header of an Entry
    async fn deregister_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()>;

    /// Returns all the [HeaderHash]es of headers that created this [Entry]
    fn get_headers<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        entry_hash: EntryHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all headers registered on an agent's public key
    fn get_activity<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [EntryUpdate] headers registered on an [Entry]
    fn get_updates<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        hash: AnyDhtHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [ElementDelete] headers registered on a Header
    fn get_deletes_on_header<'r, R: Readable>(
        &'r self,
        reader: &'r R,
        new_entry_header: HeaderHash,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>;

    /// Returns all the hashes of [ElementDelete] headers registered on an Entry's header
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

    /// Finds if there is a StoreElement under this header
    async fn has_element_header(&self, hash: &HeaderHash) -> DatabaseResult<bool>;

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
    pub fn vault(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Self::new_vault(env, dbs)
    }

    /// Create a [MetadataBuf] with the cache databases
    pub fn cache(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*CACHE_SYSTEM_META)?;
        let links_meta = dbs.get_db(&*CACHE_LINKS_META)?;
        let misc_meta = dbs.get_db(&*CACHE_STATUS_META)?;
        Self::new(env, system_meta, links_meta, misc_meta)
    }
}

impl MetadataBuf<PendingPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the PendingPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn pending(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Self::new_vault(env, dbs)
    }
}

impl MetadataBuf<JudgedPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the JudgedPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn judged(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Self::new_vault(env, dbs)
    }
}

impl MetadataBuf<RejectedPrefix> {
    /// Create a [MetadataBuf] with the vault databases using the RejectedPrefix.
    /// The data in the type will be separate from the other prefixes even though the
    /// database is shared.
    pub fn rejected(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Self::new_vault(env, dbs)
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

    fn new_vault(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let system_meta = dbs.get_db(&*META_VAULT_SYS)?;
        let links_meta = dbs.get_db(&*META_VAULT_LINKS)?;
        let misc_meta = dbs.get_db(&*META_VAULT_MISC)?;
        Self::new(env, system_meta, links_meta, misc_meta)
    }

    async fn register_header_on_basis<K, H>(&mut self, key: K, header: H) -> DatabaseResult<()>
    where
        H: Into<EntryHeader>,
        K: Into<SysMetaKey>,
    {
        let sys_val = match header.into() {
            h @ EntryHeader::NewEntry(_) => SysMetaVal::NewEntry(h.into_hash().await?),
            h @ EntryHeader::Update(_) => SysMetaVal::Update(h.into_hash().await?),
            h @ EntryHeader::Delete(_) => SysMetaVal::Delete(h.into_hash().await?),
            h @ EntryHeader::Activity(_) => SysMetaVal::Activity(h.into_hash().await?),
        };
        let key: SysMetaKey = key.into();
        self.system_meta.insert(PrefixBytesKey::new(key), sys_val);
        Ok(())
    }

    async fn deregister_header_on_basis<K, H>(&mut self, key: K, header: H) -> DatabaseResult<()>
    where
        H: Into<EntryHeader>,
        K: Into<SysMetaKey>,
    {
        let sys_val = match header.into() {
            h @ EntryHeader::NewEntry(_) => SysMetaVal::NewEntry(h.into_hash().await?),
            h @ EntryHeader::Update(_) => SysMetaVal::Update(h.into_hash().await?),
            h @ EntryHeader::Delete(_) => SysMetaVal::Delete(h.into_hash().await?),
            h @ EntryHeader::Activity(_) => SysMetaVal::Activity(h.into_hash().await?),
        };
        let key: SysMetaKey = key.into();
        self.system_meta.delete(PrefixBytesKey::new(key), sys_val);
        Ok(())
    }

    #[instrument(skip(self))]
    fn update_entry_dht_status(&mut self, basis: EntryHash) -> DatabaseResult<()> {
        let status = fresh_reader!(self.env, |r| self.get_headers(&r, basis.clone())?.find_map(
            |header| {
                if let None = self.get_deletes_on_header(&r, header.header_hash)?.next()? {
                    debug!("found live header");
                    Ok(Some(EntryDhtStatus::Live))
                } else {
                    debug!("found dead header");
                    Ok(None)
                }
            }
        ))?
        // No evidence of life found so entry is marked dead
        .unwrap_or(EntryDhtStatus::Dead);
        self.misc_meta.put(
            MiscMetaKey::EntryStatus(basis).into(),
            MiscMetaValue::EntryStatus(status),
        )
    }

    #[cfg(test)]
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.links_meta.clear_all(writer)?;
        self.system_meta.clear_all(writer)
    }
}

#[async_trait::async_trait]
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

    #[allow(clippy::needless_lifetimes)]
    async fn add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        // Register the add link onto the base
        let link_add_hash =
            HeaderHashed::from_content_sync(Header::LinkAdd(link_add.clone())).into_hash();

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

    #[allow(clippy::needless_lifetimes)]
    async fn deregister_add_link(&mut self, link_add: LinkAdd) -> DatabaseResult<()> {
        let link_add_hash = HeaderHash::with_data(&Header::LinkAdd(link_add.clone())).await;
        let key = LinkMetaKey::from((&link_add, &link_add_hash));
        self.links_meta.delete(key.into())
    }

    async fn remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()> {
        let link_add_address = link_remove.link_add_address.clone();
        // Register the link remove address to the link add address
        let link_remove = HeaderHashed::from_content_sync(Header::LinkRemove(link_remove));
        let sys_val = SysMetaVal::LinkRemove(link_remove.into());
        self.system_meta
            .insert(SysMetaKey::from(link_add_address).into(), sys_val);
        Ok(())
    }

    async fn deregister_remove_link(&mut self, link_remove: LinkRemove) -> DatabaseResult<()> {
        let link_add_address = link_remove.link_add_address.clone();
        // Register the link remove address to the link add address
        let link_remove = HeaderHashed::from_content(Header::LinkRemove(link_remove)).await;
        let sys_val = SysMetaVal::LinkRemove(link_remove.into());
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

    fn register_raw_on_header(&mut self, header_hash: HeaderHash, value: SysMetaVal) {
        self.system_meta
            .insert(SysMetaKey::from(header_hash).into(), value);
    }

    async fn register_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        self.register_header_on_basis(basis.clone(), new_entry_header)
            .await?;
        self.update_entry_dht_status(basis)?;
        Ok(())
    }

    async fn deregister_header(&mut self, new_entry_header: NewEntryHeader) -> DatabaseResult<()> {
        let basis = new_entry_header.entry().clone();
        self.deregister_header_on_basis(basis.clone(), new_entry_header)
            .await?;
        self.update_entry_dht_status(basis)?;
        Ok(())
    }

    async fn register_element_header(&mut self, header: &Header) -> DatabaseResult<()> {
        self.misc_meta.put(
            MiscMetaKey::StoreElement(HeaderHash::with_data(header).await).into(),
            MiscMetaValue::new_store_element(),
        )
    }

    async fn deregister_element_header(&mut self, hash: HeaderHash) -> DatabaseResult<()> {
        self.misc_meta
            .delete(MiscMetaKey::StoreElement(hash).into())
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()> {
        self.register_header_on_basis(
            AnyDhtHash::from(update.original_entry_address.clone()),
            update,
        )
        .await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn deregister_update(&mut self, update: header::EntryUpdate) -> DatabaseResult<()> {
        self.deregister_header_on_basis(
            AnyDhtHash::from(update.original_entry_address.clone()),
            update,
        )
        .await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()> {
        let remove = delete.removes_address.to_owned();
        let entry_hash = delete.removes_entry_address.clone();
        self.register_header_on_basis(remove, delete.clone())
            .await?;
        self.register_header_on_basis(entry_hash.clone(), delete)
            .await?;
        self.update_entry_dht_status(entry_hash)
    }

    #[allow(clippy::needless_lifetimes)]
    async fn deregister_delete(&mut self, delete: header::ElementDelete) -> DatabaseResult<()> {
        let remove = delete.removes_address.to_owned();
        let entry_hash = delete.removes_entry_address.clone();
        self.deregister_header_on_basis(remove, delete.clone())
            .await?;
        self.deregister_header_on_basis(entry_hash.clone(), delete)
            .await?;
        self.update_entry_dht_status(entry_hash)
    }

    #[allow(clippy::needless_lifetimes)]
    async fn register_activity(&mut self, header: Header) -> DatabaseResult<()> {
        let author = header.author().clone();
        self.register_header_on_basis(author, EntryHeader::Activity(header))
            .await
    }

    #[allow(clippy::needless_lifetimes)]
    async fn deregister_activity(&mut self, header: Header) -> DatabaseResult<()> {
        let author = header.author().clone();
        self.deregister_header_on_basis(author, EntryHeader::Activity(header))
            .await
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
        agent_pubkey: AgentPubKey,
    ) -> DatabaseResult<Box<dyn FallibleIterator<Item = TimedHeaderHash, Error = DatabaseError> + '_>>
    {
        Ok(Box::new(
            fallible_iterator::convert(
                self.system_meta
                    .get(r, &SysMetaKey::from(agent_pubkey).into())?,
            )
            .filter_map(|h| {
                Ok(match h {
                    SysMetaVal::Activity(h) => Some(h),
                    _ => None,
                })
            }),
        ))
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
            .get(r, &MiscMetaKey::EntryStatus(entry_hash.clone()).into())?
            .map(MiscMetaValue::entry_status)
            .unwrap_or(EntryDhtStatus::Dead))
    }

    fn get_canonical_entry_hash(&self, _entry_hash: EntryHash) -> DatabaseResult<EntryHash> {
        todo!()
    }

    fn get_canonical_header_hash(&self, _header_hash: HeaderHash) -> DatabaseResult<HeaderHash> {
        todo!()
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
                    SysMetaVal::LinkRemove(h) => Some(h),
                    _ => None,
                })
            }),
        ))
    }

    async fn has_element_header(&self, hash: &HeaderHash) -> DatabaseResult<bool> {
        fresh_reader!(self.env, |r| self
            .misc_meta
            .contains(&r, &MiscMetaKey::StoreElement(hash.clone()).into()))
    }

    fn env(&self) -> &EnvironmentRead {
        &self.env
    }
}

impl<P: PrefixType> BufferedStore for MetadataBuf<P> {
    type Error = DatabaseError;

    fn flush_to_txn(self, writer: &mut Writer) -> DatabaseResult<()> {
        self.system_meta.flush_to_txn(writer)?;
        self.links_meta.flush_to_txn(writer)?;
        self.misc_meta.flush_to_txn(writer)?;
        Ok(())
    }
}
