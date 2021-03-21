/// A convenient composition of CasBufFreshSyncs representing source chain data.
///
/// Source chain data is split into three databases: one for headers, and two
/// for public and private entries. Specifying the private_entries DB in a
/// ElementBuf is optional, so that if it's not supplied, the ElementBuf
/// will not be able to access private data. This restriction is useful when
/// using the ElementBuf for caching non-authored data, or for situations where
/// it is known that private entries should be protected, such as when handling
/// a get_entry request from the network.
use crate::source_chain::SourceChainResult;
use holo_hash::hash_type::AnyDht;
use holo_hash::AnyDhtHash;
use holo_hash::EntryHash;
use holo_hash::HasHash;
use holo_hash::HeaderHash;
use holochain_sqlite::buffer::CasBufFreshSync;
use holochain_sqlite::error::DatabaseError;
use holochain_sqlite::error::DatabaseResult;
use holochain_sqlite::exports::SingleTable;
use holochain_sqlite::prelude::*;
use holochain_types::prelude::*;
use tracing::*;

/// A CasBufFresh with Entries for values
pub type EntryCas<P> = CasBufFreshSync<Entry, P>;
/// A CasBufFresh with SignedHeaders for values
pub type HeaderCas<P> = CasBufFreshSync<SignedHeader, P>;

/// The representation of an ElementCache / ElementVault,
/// using two or three DB references
pub struct ElementBuf<P = IntegratedPrefix>
where
    P: PrefixType,
{
    public_entries: EntryCas<P>,
    private_entries: Option<EntryCas<P>>,
    headers: HeaderCas<P>,
}

impl ElementBuf<IntegratedPrefix> {
    /// Create a ElementBuf using the Vault databases.
    /// The `allow_private` argument allows you to specify whether private
    /// entries should be readable or writeable with this reference.
    /// The vault is constructed with the IntegratedPrefix.
    pub fn vault(env: EnvRead, allow_private: bool) -> DatabaseResult<Self> {
        ElementBuf::new_vault(env, allow_private)
    }

    /// Create a ElementBuf using the Cache databases.
    /// There is no cache for private entries, so private entries are disallowed
    pub fn cache(env: EnvRead) -> DatabaseResult<Self> {
        let entries = env.get_table(TableName::ElementCacheEntries)?;
        let headers = env.get_table(TableName::ElementCacheHeaders)?;
        ElementBuf::new(env, entries, None, headers)
    }
}

impl ElementBuf<PendingPrefix> {
    /// Create a element buf for all elements pending validation.
    /// This reuses the database but is the data is completely separate.
    pub fn pending(env: EnvRead) -> DatabaseResult<Self> {
        ElementBuf::new_vault(env, true)
    }
}

impl ElementBuf<RejectedPrefix> {
    /// Create a element buf for all elements that have been rejected.
    /// This reuses the database but is the data is completely separate.
    pub fn rejected(env: EnvRead) -> DatabaseResult<Self> {
        ElementBuf::new_vault(env, true)
    }
}

impl ElementBuf<AuthoredPrefix> {
    /// Create a element buf for all authored elements.
    /// This reuses the database but is the data is completely separate.
    pub fn authored(env: EnvRead, allow_private: bool) -> DatabaseResult<Self> {
        ElementBuf::new_vault(env, allow_private)
    }
}

impl<P> ElementBuf<P>
where
    P: PrefixType,
{
    fn new(
        env: EnvRead,
        public_entries_store: SingleTable,
        private_entries_store: Option<SingleTable>,
        headers_store: SingleTable,
    ) -> DatabaseResult<Self> {
        let private_entries = if let Some(store) = private_entries_store {
            Some(CasBufFreshSync::new(env.clone(), store))
        } else {
            None
        };
        Ok(Self {
            public_entries: CasBufFreshSync::new(env.clone(), public_entries_store),
            private_entries,
            headers: CasBufFreshSync::new(env, headers_store),
        })
    }

    /// Construct a element buf using the vault databases
    fn new_vault(env: EnvRead, allow_private: bool) -> DatabaseResult<Self> {
        let headers = env.get_table(TableName::ElementVaultHeaders)?;
        let entries = env.get_table(TableName::ElementVaultPublicEntries)?;
        let private_entries = if allow_private {
            Some(env.get_table(TableName::ElementVaultPrivateEntries)?)
        } else {
            None
        };
        Self::new(env, entries, private_entries, headers)
    }

    /// Get an entry by its address
    ///
    /// First attempt to get from the public entry DB. If not present, and
    /// private DB access is specified, attempt to get as a private entry.
    pub fn get_entry(&self, entry_hash: &EntryHash) -> DatabaseResult<Option<EntryHashed>> {
        match self.public_entries.get(entry_hash)? {
            Some(entry) => Ok(Some(entry)),
            None => {
                if let Some(ref db) = (self).private_entries {
                    db.get(entry_hash)
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub fn contains_entry(&self, entry_hash: &EntryHash) -> DatabaseResult<bool> {
        Ok(if self.public_entries.contains(entry_hash)? {
            true
        } else {
            // Potentially avoid this let Some if the above branch is hit first
            if let Some(private) = &self.private_entries {
                private.contains(entry_hash)?
            } else {
                false
            }
        })
    }

    pub fn contains_header(&self, header_hash: &HeaderHash) -> DatabaseResult<bool> {
        self.headers.contains(header_hash)
    }

    pub fn contains_in_scratch(&self, hash: &AnyDhtHash) -> DatabaseResult<bool> {
        match *hash.hash_type() {
            AnyDht::Entry => {
                Ok(
                    if self
                        .public_entries
                        .contains_in_scratch(&hash.clone().into())?
                    {
                        true
                    } else {
                        // Potentially avoid this let Some if the above branch is hit first
                        if let Some(private) = &self.private_entries {
                            private.contains_in_scratch(&hash.clone().into())?
                        } else {
                            false
                        }
                    },
                )
            }
            AnyDht::Header => self.headers.contains_in_scratch(&hash.clone().into()),
        }
    }

    pub fn get_header(
        &self,
        header_address: &HeaderHash,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        Ok(self.headers.get(header_address)?.map(Into::into))
    }

    pub fn get_header_with_reader<'r, 'a: 'r, R: Readable>(
        &'a self,
        r: &'r mut R,
        header_address: &HeaderHash,
    ) -> DatabaseResult<Option<SignedHeaderHashed>> {
        Ok(self.headers.inner().get(r, header_address)?.map(Into::into))
    }

    /// Get the Entry out of Header if it exists.
    ///
    /// If the header contains no entry data, return None
    /// If the header contains entry data:
    /// - if it is a public entry, but the entry cannot be found, return error
    /// - if it is a private entry and cannot be found, return error
    /// - if it is a private entry but the private DB is disabled, return None
    fn get_entry_from_header(&self, header: &Header) -> SourceChainResult<Option<Entry>> {
        Ok(match header.entry_data() {
            None => None,
            Some((entry_hash, entry_type)) => {
                match entry_type.visibility() {
                    // if the header references an entry and the database is
                    // available, it better have been stored!
                    EntryVisibility::Public => self.public_entries.get(entry_hash)?,
                    EntryVisibility::Private => {
                        if let Some(ref db) = self.private_entries {
                            db.get(entry_hash)?
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
    pub fn get_element(&self, header_address: &HeaderHash) -> SourceChainResult<Option<Element>> {
        if let Some(signed_header) = self.get_header(header_address)? {
            let maybe_entry = self.get_entry_from_header(signed_header.header())?;
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
                            error!(
                                "Attempted ElementBuf::put on a private entry with a disabled private DB: {}",
                                entry.as_hash()
                            );
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

    pub fn put_element_group(&mut self, element_group: ElementGroup) -> SourceChainResult<()> {
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
                    error!(
                        "Attempted ElementBuf::put on a private entry with a disabled private DB: {}",
                        entry.as_hash()
                    );
                }
            }
        }
        Ok(())
    }

    pub fn delete(&mut self, header_hash: HeaderHash, entry_hash: Option<EntryHash>) {
        self.headers.delete(header_hash);
        if let Some(entry_hash) = entry_hash {
            if let Some(db) = self.private_entries.as_mut() {
                db.delete(entry_hash.clone())
            }
            self.public_entries.delete(entry_hash);
        }
    }

    /// Removes a delete if there was one previously added
    pub fn cancel_delete(&mut self, header_hash: HeaderHash, entry_hash: Option<EntryHash>) {
        self.headers.cancel_delete(header_hash);
        if let Some(entry_hash) = entry_hash {
            if let Some(db) = self.private_entries.as_mut() {
                db.cancel_delete(entry_hash.clone())
            }
            self.public_entries.cancel_delete(entry_hash);
        }
    }

    pub fn headers(&self) -> &HeaderCas<P> {
        &self.headers
    }

    pub fn public_entries(&self) -> &EntryCas<P> {
        &self.public_entries
    }

    pub fn private_entries(&self) -> Option<&EntryCas<P>> {
        self.private_entries.as_ref()
    }

    #[cfg(any(test, feature = "test_utils"))]
    /// Clear all scratch and db, useful for tests
    pub fn clear_all(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        self.public_entries.clear_all(writer)?;
        if let Some(private) = &mut self.private_entries {
            private.clear_all(writer)?
        }
        self.headers.clear_all(writer)
    }
}

impl<P: PrefixType> BufferedStore for ElementBuf<P> {
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

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> DatabaseResult<()> {
        if self.is_clean() {
            return Ok(());
        }
        self.public_entries.flush_to_txn_ref(writer)?;
        if let Some(ref mut db) = self.private_entries {
            db.flush_to_txn_ref(writer)?
        };
        self.headers.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::test_cell_env;

    use super::ElementBuf;
    use holo_hash::*;
    use holochain_keystore::test_keystore::spawn_test_keystore;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_sqlite::prelude::*;
    use holochain_types::test_utils::fake_unique_element;
    use holochain_zome_types::entry_def::EntryVisibility;

    #[tokio::test(flavor = "multi_thread")]
    async fn can_write_private_entry_when_enabled() -> anyhow::Result<()> {
        let keystore = spawn_test_keystore().await?;
        let test_env = test_cell_env();
        let arc = test_env.env();

        let agent_key = AgentPubKey::new_from_pure_entropy(&keystore).await?;
        let (header_pub, entry_pub) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Public).await?;
        let (header_priv, entry_priv) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Private).await?;

        // write one public-entry header and one private-entry header
        arc.conn().unwrap().with_commit(|txn| {
            let mut store = ElementBuf::vault(arc.clone().into(), true)?;
            store.put(header_pub, Some(entry_pub.clone()))?;
            store.put(header_priv, Some(entry_priv.clone()))?;
            store.flush_to_txn(txn)
        })?;

        // Can retrieve both entries when private entries are enabled
        {
            let store = ElementBuf::vault(arc.clone().into(), true)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()),
                Ok(Some(entry_pub.clone()))
            );
            assert_eq!(
                store.get_entry(entry_priv.as_hash()),
                Ok(Some(entry_priv.clone()))
            );
        }

        // Cannot retrieve private entry when disabled
        {
            let store = ElementBuf::vault(arc.clone().into(), false)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()),
                Ok(Some(entry_pub.clone()))
            );
            assert_eq!(store.get_entry(entry_priv.as_hash()), Ok(None));
        }

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cannot_write_private_entry_when_disabled() -> anyhow::Result<()> {
        let keystore = spawn_test_keystore().await?;
        let test_env = test_cell_env();
        let arc = test_env.env();

        let agent_key = AgentPubKey::new_from_pure_entropy(&keystore).await?;
        let (header_pub, entry_pub) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Public).await?;
        let (header_priv, entry_priv) =
            fake_unique_element(&keystore, agent_key.clone(), EntryVisibility::Private).await?;

        // write one public-entry header and one private-entry header (which will be a noop)
        arc.conn().unwrap().with_commit(|txn| {
            let mut store = ElementBuf::vault(arc.clone().into(), false)?;
            store.put(header_pub, Some(entry_pub.clone()))?;
            store.put(header_priv, Some(entry_priv.clone()))?;
            store.flush_to_txn(txn)
        })?;

        // Can retrieve both entries when private entries are enabled
        {
            let store = ElementBuf::vault(arc.clone().into(), true)?;
            assert_eq!(
                store.get_entry(entry_pub.as_hash()),
                Ok(Some(entry_pub.clone()))
            );
            assert_eq!(store.get_entry(entry_priv.as_hash()), Ok(None));
        }

        // Cannot retrieve private entry when disabled
        {
            let store = ElementBuf::vault(arc.clone().into(), false)?;
            assert_eq!(store.get_entry(entry_pub.as_hash()), Ok(Some(entry_pub)));
            assert_eq!(store.get_entry(entry_priv.as_hash()), Ok(None));
        }

        Ok(())
    }
}

/// Create an ElementBuf with a clone of the scratch
/// from another ElementBuf
impl<P> From<&ElementBuf<P>> for ElementBuf<P>
where
    P: PrefixType,
{
    fn from(other: &ElementBuf<P>) -> Self {
        Self {
            public_entries: (&other.public_entries).into(),
            private_entries: other.private_entries.as_ref().map(|pe| pe.into()),
            headers: (&other.headers).into(),
        }
    }
}
