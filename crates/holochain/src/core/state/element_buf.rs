/// A convenient composition of CasBufFreshsepresenting source chain data.
///
/// Source chain data is split into three databases: one for headers, and two
/// for public and private entries. Specifying the private_entries DB in a
/// ElementBuf is optional, so that if it's not supplied, the ElementBuf
/// will not be able to access private data. This restriction is useful when
/// using the ElementBuf for caching non-authored data, or for situations where
/// it is known that private entries should be protected, such as when handling
/// a get_entry request from the network.
use crate::core::state::source_chain::{ChainInvalidReason, SourceChainError, SourceChainResult};
use holo_hash::{EntryHash, HasHash, HeaderHash};
use holochain_state::{
    buffer::CasBufFresh,
    db::{
        GetDb, ELEMENT_CACHE_ENTRIES, ELEMENT_CACHE_HEADERS, ELEMENT_VAULT_HEADERS,
        ELEMENT_VAULT_PRIVATE_ENTRIES, ELEMENT_VAULT_PUBLIC_ENTRIES,
    },
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    prelude::*,
};
use holochain_types::{
    element::{Element, ElementGroup, SignedHeader, SignedHeaderHashed},
    entry::EntryHashed,
};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::{Entry, Header};
use tracing::*;

/// A CasBufFresh with Entries for values
pub type EntryCas = CasBufFresh<Entry>;
/// A CasBufFresh with SignedHeaders for values
pub type HeaderCas = CasBufFresh<SignedHeader>;

/// The representation of an ElementCache / ElementVault,
/// using two or three DB references
pub struct ElementBuf {
    public_entries: EntryCas,
    private_entries: Option<EntryCas>,
    headers: HeaderCas,
}

impl ElementBuf {
    fn new(
        env: EnvironmentRead,
        public_entries_store: SingleStore,
        private_entries_store: Option<SingleStore>,
        headers_store: SingleStore,
    ) -> DatabaseResult<Self> {
        let private_entries = if let Some(store) = private_entries_store {
            Some(CasBufFresh::new(env.clone(), store))
        } else {
            None
        };
        Ok(Self {
            public_entries: CasBufFresh::new(env.clone(), public_entries_store),
            private_entries,
            headers: CasBufFresh::new(env, headers_store),
        })
    }

    /// Create a ElementBuf using the Vault databases.
    /// The `allow_private` argument allows you to specify whether private
    /// entries should be readable or writeable with this reference.
    pub fn vault(
        env: EnvironmentRead,
        dbs: &impl GetDb,
        allow_private: bool,
    ) -> DatabaseResult<Self> {
        let headers = dbs.get_db(&*ELEMENT_VAULT_HEADERS)?;
        let entries = dbs.get_db(&*ELEMENT_VAULT_PUBLIC_ENTRIES)?;
        let private_entries = if allow_private {
            Some(dbs.get_db(&*ELEMENT_VAULT_PRIVATE_ENTRIES)?)
        } else {
            None
        };
        Self::new(env, entries, private_entries, headers)
    }

    /// Create a ElementBuf using the Cache databases.
    /// There is no cache for private entries, so private entries are disallowed
    pub fn cache(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        let entries = dbs.get_db(&*ELEMENT_CACHE_ENTRIES)?;
        let headers = dbs.get_db(&*ELEMENT_CACHE_HEADERS)?;
        Self::new(env, entries, None, headers)
    }

    /// Get an entry by its address
    ///
    /// First attempt to get from the public entry DB. If not present, and
    /// private DB access is specified, attempt to get as a private entry.
    pub async fn get_entry(&self, entry_hash: &EntryHash) -> DatabaseResult<Option<EntryHashed>> {
        match self.public_entries.get(entry_hash).await? {
            Some(entry) => Ok(Some(entry)),
            None => {
                if let Some(ref db) = (self).private_entries {
                    db.get(entry_hash).await
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub async fn contains_entry(&self, entry_hash: &EntryHash) -> DatabaseResult<bool> {
        Ok(if self.public_entries.contains(entry_hash).await? {
            true
        } else {
            // Potentially avoid this let Some if the above branch is hit first
            if let Some(private) = &self.private_entries {
                private.contains(entry_hash).await?
            } else {
                false
            }
        })
    }

    pub async fn contains_header(&self, header_hash: &HeaderHash) -> DatabaseResult<bool> {
        self.headers.contains(header_hash).await
    }

    pub async fn get_header(
        &self,
        header_address: &HeaderHash,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        Ok(self.headers.get(header_address).await?.map(Into::into))
    }

    /// Get the Entry out of Header if it exists.
    ///
    /// If the header contains no entry data, return None
    /// If the header contains entry data:
    /// - if it is a public entry, but the entry cannot be found, return error
    /// - if it is a private entry and cannot be found, return error
    /// - if it is a private entry but the private DB is disabled, return None
    async fn get_entry_from_header(&self, header: &Header) -> SourceChainResult<Option<Entry>> {
        Ok(match header.entry_data() {
            None => None,
            Some((entry_hash, entry_type)) => {
                match entry_type.visibility() {
                    // if the header references an entry and the database is
                    // available, it better have been stored!
                    EntryVisibility::Public => {
                        Some(self.public_entries.get(entry_hash).await?.ok_or_else(|| {
                            SourceChainError::InvalidStructure(ChainInvalidReason::MissingData(
                                entry_hash.clone(),
                            ))
                        })?)
                    }
                    EntryVisibility::Private => {
                        if let Some(ref db) = self.private_entries {
                            Some(db.get(entry_hash).await?.ok_or_else(|| {
                                SourceChainError::InvalidStructure(ChainInvalidReason::MissingData(
                                    entry_hash.clone(),
                                ))
                            })?)
                        } else {
                            // If the private DB is disabled, just return None
                            None
                        }
                    }
                }
            }
        }
        .map(|e| e.into_content()))
    }

    /// given a header address return the full chain element for that address
    pub async fn get_element(
        &self,
        header_address: &HeaderHash,
    ) -> SourceChainResult<Option<Element>> {
        if let Some(signed_header) = self.get_header(header_address).await? {
            let maybe_entry = self.get_entry_from_header(signed_header.header()).await?;
            Ok(Some(Element::new(signed_header, maybe_entry)))
        } else {
            Ok(None)
        }
    }

    /// Puts a signed header and optional entry into the Element store.
    /// N.B. this code assumes that the header and entry have been validated
    pub fn put(
        &mut self,
        signed_header: SignedHeaderHashed,
        maybe_entry: Option<EntryHashed>,
    ) -> DatabaseResult<()> {
        if let Some(entry) = maybe_entry {
            if let Some((_, entry_type)) = signed_header.header().entry_data() {
                match entry_type.visibility() {
                    EntryVisibility::Public => self.public_entries.put(entry),
                    EntryVisibility::Private => {
                        if let Some(db) = self.private_entries.as_mut() {
                            db.put(entry);
                        } else {
                            error!("Attempted ElementBuf::put on a private entry with a disabled private DB: {}", entry.as_hash());
                        }
                    }
                }
            } else {
                unreachable!(
                    "Attempting to put an entry, but the header has no entry_type. Header hash: {}",
                    signed_header.header_address()
                );
            }
        }

        self.headers.put(signed_header.into());
        Ok(())
    }

    pub fn put_element_group(&mut self, element_group: ElementGroup) -> DatabaseResult<()> {
        for shh in element_group.owned_signed_headers() {
            self.headers.put(shh.into());
        }
        let entry = element_group.entry_hashed();
        match element_group.visibility()? {
            EntryVisibility::Public => self.public_entries.put(entry),
            EntryVisibility::Private => {
                if let Some(db) = self.private_entries.as_mut() {
                    db.put(entry);
                } else {
                    error!("Attempted ElementBuf::put on a private entry with a disabled private DB: {}", entry.as_hash());
                }
            }
        }
        Ok(())
    }

    pub fn delete(&mut self, header_hash: HeaderHash, entry_hash: EntryHash) {
        self.headers.delete(header_hash);
        if let Some(db) = self.private_entries.as_mut() {
            db.delete(entry_hash.clone())
        }
        self.public_entries.delete(entry_hash);
    }

    pub fn headers(&self) -> &HeaderCas {
        &self.headers
    }

    pub fn public_entries(&self) -> &EntryCas {
        &self.public_entries
    }

    pub fn private_entries(&self) -> Option<&EntryCas> {
        self.private_entries.as_ref()
    }

    #[cfg(test)]
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.public_entries.clear_all(writer)?;
        if let Some(private) = &mut self.private_entries {
            private.clear_all(writer)?
        }
        self.headers.clear_all(writer)
    }
}

impl BufferedStore for ElementBuf {
    type Error = DatabaseError;

    fn is_clean(&self) -> bool {
        self.headers.is_clean()
            && self.public_entries.is_clean()
            && self
                .private_entries
                .as_ref()
                .map(|db| db.is_clean())
                .unwrap_or(true)
    }

    fn flush_to_txn(self, writer: &mut Writer) -> DatabaseResult<()> {
        if self.is_clean() {
            return Ok(());
        }
        self.public_entries.flush_to_txn(writer)?;
        if let Some(db) = self.private_entries {
            db.flush_to_txn(writer)?
        };
        self.headers.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::ElementBuf;
    use crate::test_utils::fake_unique_element;
    use holo_hash::*;
    use holochain_keystore::test_keystore::spawn_test_keystore;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_state::{prelude::*, test_utils::test_cell_env};
    use holochain_zome_types::entry_def::EntryVisibility;

    #[tokio::test(threaded_scheduler)]
    async fn can_write_private_entry_when_enabled() -> anyhow::Result<()> {
        let keystore = spawn_test_keystore(Vec::new()).await?;
        let arc = test_cell_env();
        let env = arc.guard().await;

        let agent_key = AgentPubKey::new_from_pure_entropy(&keystore).await?;
        let (header_pub, entry_pub) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Public).await?;
        let (header_priv, entry_priv) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Private).await?;

        // write one public-entry header and one private-entry header
        env.with_commit(|txn| {
            let _reader = env.reader()?;
            let mut store = ElementBuf::vault(arc.clone().into(), &env, true)?;
            store.put(header_pub, Some(entry_pub.clone()))?;
            store.put(header_priv, Some(entry_priv.clone()))?;
            store.flush_to_txn(txn)
        })?;

        // Can retrieve both entries when private entries are enabled
        {
            let _reader = env.reader()?;
            let store = ElementBuf::vault(arc.clone().into(), &env, true)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()).await,
                Ok(Some(entry_pub.clone()))
            );
            assert_eq!(
                store.get_entry(entry_priv.as_hash()).await,
                Ok(Some(entry_priv.clone()))
            );
        }

        // Cannot retrieve private entry when disabled
        {
            let _reader = env.reader()?;
            let store = ElementBuf::vault(arc.clone().into(), &env, false)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()).await,
                Ok(Some(entry_pub.clone()))
            );
            assert_eq!(store.get_entry(entry_priv.as_hash()).await, Ok(None));
        }

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn cannot_write_private_entry_when_disabled() -> anyhow::Result<()> {
        let keystore = spawn_test_keystore(Vec::new()).await?;
        let arc = test_cell_env();
        let env = arc.guard().await;

        let agent_key = AgentPubKey::new_from_pure_entropy(&keystore).await?;
        let (header_pub, entry_pub) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Public).await?;
        let (header_priv, entry_priv) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Private).await?;

        // write one public-entry header and one private-entry header (which will be a noop)
        env.with_commit(|txn| {
            let _reader = env.reader()?;
            let mut store = ElementBuf::vault(arc.clone().into(), &env, false)?;
            store.put(header_pub, Some(entry_pub.clone()))?;
            store.put(header_priv, Some(entry_priv.clone()))?;
            store.flush_to_txn(txn)
        })?;

        // Can retrieve both entries when private entries are enabled
        {
            let _reader = env.reader()?;
            let store = ElementBuf::vault(arc.clone().into(), &env, true)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()).await,
                Ok(Some(entry_pub.clone()))
            );
            assert_eq!(store.get_entry(entry_priv.as_hash()).await, Ok(None));
        }

        // Cannot retrieve private entry when disabled
        {
            let _reader = env.reader()?;
            let store = ElementBuf::vault(arc.clone().into(), &env, false)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()).await,
                Ok(Some(entry_pub))
            );
            assert_eq!(store.get_entry(entry_priv.as_hash()).await, Ok(None));
        }

        Ok(())
    }
}
