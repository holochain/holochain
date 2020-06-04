//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

use holo_hash::*;
use holochain_state::{
    buffer::BufferedStore,
    db::GetDb,
    error::DatabaseResult,
    prelude::{Reader, Writer},
};
use holochain_types::{
    composite_hash::HeaderAddress,
    header::{builder, EntryType, HeaderBuilderCommon, HeaderInner},
    prelude::*,
    EntryHashed,
};
use holochain_zome_types::{
    capability::{CapClaim, CapGrant, CapSecret},
    entry::{CapClaimEntry, CapGrantEntry, Entry},
};
use shrinkwraprs::Shrinkwrap;

use builder::HeaderBuilder;
pub use error::*;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct SourceChain<'env>(pub SourceChainBuf<'env>);

impl<'env> SourceChain<'env> {
    pub fn agent_pubkey(&self) -> SourceChainResult<AgentPubKey> {
        self.0
            .agent_pubkey()?
            .ok_or(SourceChainError::InvalidStructure(
                ChainInvalidReason::GenesisDataMissing,
            ))
    }

    pub fn chain_head(&self) -> SourceChainResult<&HeaderAddress> {
        self.0.chain_head().ok_or(SourceChainError::ChainEmpty)
    }

    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(reader, dbs)?.into())
    }

    pub fn into_inner(self) -> SourceChainBuf<'env> {
        self.0
    }

    /// Add a ChainElement to the source chain, using a HeaderBuilder
    pub async fn put<H: HeaderInner, B: HeaderBuilder<H>>(
        &mut self,
        header_builder: B,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderAddress> {
        let common = HeaderBuilderCommon {
            author: self.agent_pubkey()?,
            timestamp: Timestamp::now(),
            header_seq: self.len() as u32,
            prev_header: self.chain_head()?.to_owned(),
        };
        let header = header_builder.build_header(common);
        self.put_raw(header, maybe_entry).await
    }

    /// Add a CapGrantEntry to the source chain
    pub async fn put_cap_grant(
        &mut self,
        grant_entry: CapGrantEntry,
    ) -> SourceChainResult<HeaderAddress> {
        let (entry, entry_hash) = EntryHashed::with_data(Entry::CapGrant(grant_entry))
            .await?
            .into_inner();
        let header_builder = builder::EntryCreate {
            entry_type: EntryType::CapGrant,
            entry_hash,
        };
        self.put(header_builder, Some(entry)).await
    }

    /// Add a CapClaimEntry to the source chain
    pub async fn put_cap_claim(
        &mut self,
        claim_entry: CapClaimEntry,
    ) -> SourceChainResult<HeaderAddress> {
        let (entry, entry_hash) = EntryHashed::with_data(Entry::CapClaim(claim_entry))
            .await?
            .into_inner();
        let header_builder = builder::EntryCreate {
            entry_type: EntryType::CapClaim,
            entry_hash,
        };
        self.put(header_builder, Some(entry)).await
    }

    /// Fetch a CapGrant from the private entries.
    ///
    /// NB: [B-01676] the entry must be persisted for this to work. Once we have a
    /// proper capability index DB, OR a proper iterator that respects the
    /// scratch space, that will no longer be the case.
    pub fn get_persisted_cap_grant_by_secret(
        &self,
        query: &CapSecret,
    ) -> SourceChainResult<Option<CapGrant>> {
        let hashes_n_grants: Vec<_> = self
            .0
            .cas()
            .private_entries()
            .expect(
                "SourceChainBuf must have access to private entries in order to access CapGrants",
            )
            .iter_raw()?
            .filter_map(|entry| {
                entry.as_cap_grant().and_then(|grant| {
                    grant.access().secret().and_then(|secret| {
                        if secret == query {
                            Some((entry.into_hash(), grant.clone()))
                        } else {
                            None
                        }
                    })
                })
            })
            .collect();

        let answer = if hashes_n_grants.len() == 0 {
            None
        } else if hashes_n_grants.len() == 1 {
            hashes_n_grants.first().map(|p| p.1.clone())
        } else {
            // FIXME[B-01676]: we SHOULD iterate through the chain now to find the most
            // recent grant with this secret, in case it was updated.
            // This will be handled in the future with an index, for simple
            // lookup by secret
            todo!("Find proper grant or implement capability index")
        };
        Ok(answer)
    }

    /// Fetch a CapClaim from the private entries.
    ///
    /// NB: [B-01676] the entry must be persisted for this to work. Once we have a
    /// proper capability index DB, OR a proper iterator that respects the
    /// scratch space, that will no longer be the case.
    pub fn get_persisted_cap_claim_by_secret(
        &self,
        query: &CapSecret,
    ) -> SourceChainResult<Option<CapClaim>> {
        let hashes_n_claims: Vec<_> = self
            .0
            .cas()
            .private_entries()
            .expect(
                "SourceChainBuf must have access to private entries in order to access CapClaims",
            )
            .iter_raw()?
            .filter_map(|entry| {
                entry.clone().as_cap_claim().and_then(|claim| {
                    if claim.secret() == query {
                        Some((entry.into_hash(), claim.clone()))
                    } else {
                        None
                    }
                })
            })
            .collect();

        let answer = if hashes_n_claims.len() == 0 {
            None
        } else if hashes_n_claims.len() == 1 {
            hashes_n_claims.first().map(|p| p.1.clone())
        } else {
            // FIXME[B-01676]: we SHOULD iterate through the chain now to find the most
            // recent claim with this secret, in case it was updated.
            // This will be handled in the future with an index, for simple
            // lookup by secret
            todo!("Find proper claim or implement capability index")
        };
        Ok(answer)
    }
}

impl<'env> From<SourceChainBuf<'env>> for SourceChain<'env> {
    fn from(buffer: SourceChainBuf<'env>) -> Self {
        Self(buffer)
    }
}

impl<'env> BufferedStore<'env> for SourceChain<'env> {
    type Error = SourceChainError;

    fn flush_to_txn(self, writer: &'env mut Writer) -> Result<(), Self::Error> {
        self.0.flush_to_txn(writer)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use holochain_state::prelude::*;
    use holochain_state::test_utils::test_cell_env;
    use holochain_types::test_utils::{fake_agent_pubkey_1, fake_dna_hash};
    use holochain_zome_types::capability::{CapAccess, ZomeCallCapGrant};
    use std::collections::BTreeMap;

    #[tokio::test(threaded_scheduler)]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let access = CapAccess::transferable();
        let secret = access.secret().unwrap();
        let grant = ZomeCallCapGrant::new("tag".into(), access.clone(), BTreeMap::new());
        {
            let reader = env.reader()?;
            let mut store = SourceChainBuf::new(&reader, &env)?;
            store
                .genesis(fake_dna_hash(""), fake_agent_pubkey_1(), None)
                .await?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;
            let mut chain = SourceChain::new(&reader, &env)?;
            chain.put_cap_grant(grant.clone()).await?;

            // ideally the following would work, but it won't because currently
            // we can't get grants from the scratch space
            // this will be fixed once we add the capability index

            // assert_eq!(
            //     chain.get_persisted_cap_grant_by_secret(secret)?,
            //     Some(grant.clone().into())
            // );

            env.with_commit(|writer| chain.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;
            let chain = SourceChain::new(&reader, &env)?;
            assert_eq!(
                chain.get_persisted_cap_grant_by_secret(secret)?,
                Some(grant.into())
            );
        }

        Ok(())
    }

    #[tokio::test(threaded_scheduler)]
    async fn test_get_cap_claim() -> SourceChainResult<()> {
        let arc = test_cell_env();
        let env = arc.guard().await;
        let secret = CapSecret::random();
        let agent_pubkey = fake_agent_pubkey_1().into();
        let claim = CapClaim::new("tag".into(), agent_pubkey, secret.clone());
        {
            let reader = env.reader()?;
            let mut store = SourceChainBuf::new(&reader, &env)?;
            store
                .genesis(fake_dna_hash(""), fake_agent_pubkey_1(), None)
                .await?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;
            let mut chain = SourceChain::new(&reader, &env)?;
            chain.put_cap_claim(claim.clone()).await?;

            // ideally the following would work, but it won't because currently
            // we can't get claims from the scratch space
            // this will be fixed once we add the capability index

            // assert_eq!(
            //     chain.get_persisted_cap_claim_by_secret(&secret)?,
            //     Some(claim.clone())
            // );

            env.with_commit(|writer| chain.flush_to_txn(writer))?;
        }

        {
            let reader = env.reader()?;
            let chain = SourceChain::new(&reader, &env)?;
            assert_eq!(
                chain.get_persisted_cap_claim_by_secret(&secret)?,
                Some(claim)
            );
        }

        Ok(())
    }
}
