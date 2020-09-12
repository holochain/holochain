//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

pub use error::*;
use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_state::{buffer::BufferedStore, error::DatabaseResult, fresh_reader, prelude::*};
use holochain_types::{prelude::*, EntryHashed, HeaderHashed};
use holochain_zome_types::capability::GrantedFunction;
use holochain_zome_types::{
    capability::{CapGrant, CapSecret},
    entry::{CapClaimEntry, CapGrantEntry, Entry},
    header::{builder, EntryType, HeaderBuilder, HeaderBuilderCommon, HeaderInner},
    query::ChainQueryFilter,
};
use shrinkwraprs::Shrinkwrap;
pub use source_chain_buffer::*;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct SourceChain(pub SourceChainBuf);

impl SourceChain {
    pub fn agent_pubkey(&self) -> SourceChainResult<AgentPubKey> {
        self.0
            .agent_pubkey()?
            .ok_or(SourceChainError::InvalidStructure(
                ChainInvalidReason::GenesisDataMissing,
            ))
    }

    pub fn chain_head(&self) -> SourceChainResult<&HeaderHash> {
        self.0.chain_head().ok_or(SourceChainError::ChainEmpty)
    }

    pub fn new(env: EnvironmentRead) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(env)?.into())
    }

    pub fn public_only(env: EnvironmentRead) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::public_only(env)?.into())
    }

    pub fn into_inner(self) -> SourceChainBuf {
        self.0
    }

    /// Add a Element to the source chain, using a HeaderBuilder
    pub async fn put<H: HeaderInner, B: HeaderBuilder<H>>(
        &mut self,
        header_builder: B,
        maybe_entry: Option<Entry>,
    ) -> SourceChainResult<HeaderHash> {
        let common = HeaderBuilderCommon {
            author: self.agent_pubkey()?,
            timestamp: Timestamp::now().into(),
            header_seq: self.len() as u32,
            prev_header: self.chain_head()?.to_owned(),
        };
        let header = header_builder.build(common).into();
        self.put_raw(header, maybe_entry).await
    }

    /// Add a CapGrantEntry to the source chain
    pub async fn put_cap_grant(
        &mut self,
        grant_entry: CapGrantEntry,
    ) -> SourceChainResult<HeaderHash> {
        let (entry, entry_hash) =
            EntryHashed::from_content_sync(Entry::CapGrant(grant_entry)).into_inner();
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
    ) -> SourceChainResult<HeaderHash> {
        let (entry, entry_hash) =
            EntryHashed::from_content_sync(Entry::CapClaim(claim_entry)).into_inner();
        let header_builder = builder::EntryCreate {
            entry_type: EntryType::CapClaim,
            entry_hash,
        };
        self.put(header_builder, Some(entry)).await
    }

    /// Fetch a relevant CapGrant from the private entries.
    ///
    /// If a function has an Unrestricted grant against it, this may be returned.
    ///
    /// Else the secret and assignees of a grant will be checked and may be returned.
    ///
    /// @todo there is no order/guarantee what grant will be returned if there are multiple matches
    /// this means that CRUD probably doesn't work for grants atm.
    ///
    /// NB: [B-01676] the entry must be persisted for this to work. Once we have a
    /// proper capability index DB, OR a proper iterator that respects the
    /// scratch space, that will no longer be the case.
    pub fn valid_cap_grant(
        &self,
        check_function: &GrantedFunction,
        check_agent: &AgentPubKey,
        check_secret: &CapSecret,
    ) -> SourceChainResult<Option<CapGrant>> {
        let author_grant = CapGrant::from(self.agent_pubkey()?);
        if author_grant.is_valid(check_function, check_agent, check_secret) {
            return Ok(Some(author_grant));
        }

        let committed_valid_grant = fresh_reader!(self.env(), |r| {
            self
            .0
            .elements()
            .private_entries()
            .expect(
                "SourceChainBuf must have access to private entries in order to access CapGrants",
            )
            .iter_fail(&r)?
            // filter all entries down to only cap grant entries
            .filter_map(|entry| {
                Ok(entry.as_cap_grant())
            })
            // filter down to only the grants for this function
            .filter(|grant| {
                Ok(grant.is_valid(check_function, check_agent, check_secret))
            })
            .next()
        })?;
        Ok(committed_valid_grant)
    }

    // @todo bring all this back when we want to administer cap claims better
    //         /// Fetch a CapClaim from the private entries.
    //         ///
    //         /// NB: [B-01676] the entry must be persisted for this to work. Once we have a
    //         /// proper capability index DB, OR a proper iterator that respects the
    //         /// scratch space, that will no longer be the case.
    //         pub async fn get_persisted_cap_claim_by_secret(
    //             &self,
    //             query: &CapSecret,
    //         ) -> SourceChainResult<Option<CapClaim>> {
    //             let hashes_n_claims: Vec<_> = fresh_reader!(self.env(), |r| {
    //                 self
    //                 .0
    //                 .elements()
    //                 .private_entries()
    //                 .expect(
    //                     "SourceChainBuf must have access to private entries in order to access CapClaims",
    //                 )
    //                 .iter_fail(&r)?
    //                 .filter_map(|entry| {
    //                     if let (Entry::CapClaim(claim), entry_hash) = entry.into_inner() {
    //                         Ok(Some((entry_hash, claim)))
    //                     } else {
    //                         Ok(None)
    //                     }
    //                 })
    //                 .filter(|(_entry_hash, claim)| Ok(claim.secret() == query))
    //                 .collect()
    //             })?;
    //
    //             let answer = if hashes_n_claims.len() == 0 {
    //                 None
    //             } else if hashes_n_claims.len() == 1 {
    //                 hashes_n_claims.first().map(|p| p.1.clone())
    //             } else {
    //                 // FIXME[B-01676]: we SHOULD iterate through the chain now to find the most
    //                 // recent claim with this secret, in case it was updated.
    //                 // This will be handled in the future with an index, for simple
    //                 // lookup by secret
    //                 todo!("Find proper claim or implement capability index")
    //             };
    //             Ok(answer)
    //         }
    //     }
    // }

    /// Query Headers in the source chain.
    /// This returns a Vec rather than an iterator because it is intended to be
    /// used by the `query` host function, which crosses the wasm boundary
    pub fn query(&self, query: &ChainQueryFilter) -> SourceChainResult<Vec<HeaderHashed>> {
        self.iter_back()
            .filter(|shh| Ok(query.check(shh.header())))
            .map(|shh| Ok(shh.header_hashed().clone()))
            .collect()
    }
}

impl From<SourceChainBuf> for SourceChain {
    fn from(buffer: SourceChainBuf) -> Self {
        Self(buffer)
    }
}

impl BufferedStore for SourceChain {
    type Error = SourceChainError;

    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> Result<(), Self::Error> {
        self.0.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use crate::fixt::*;
    use ::fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_state::test_utils::test_cell_env;
    use holochain_types::test_utils::fake_dna_hash;
    use holochain_zome_types::capability::{CapAccess, ZomeCallCapGrant};
    use std::collections::BTreeSet;

    #[tokio::test(threaded_scheduler)]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let test_env = test_cell_env();
        let env = test_env.env();
        let access = CapAccess::from(CapSecretFixturator::new(Unpredictable).next().unwrap());
        let secret = access.secret().unwrap();
        // @todo curry
        let _curry = CurryPayloadsFixturator::new(Empty).next().unwrap();
        let function: GrantedFunction = ("foo".into(), "bar".into());
        let mut functions: GrantedFunctions = BTreeSet::new();
        functions.insert(function.clone());
        let grant = ZomeCallCapGrant::new("tag".into(), access.clone(), functions);
        let mut agents = AgentPubKeyFixturator::new(Predictable);
        let alice = agents.next().unwrap();
        let bob = agents.next().unwrap();
        {
            let mut store = SourceChainBuf::new(env.clone().into())?;
            store.genesis(fake_dna_hash(1), alice.clone(), None).await?;
            env.guard()
                .with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let chain = SourceChain::new(env.clone().into())?;
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, secret)?,
                Some(CapGrant::Authorship(alice.clone())),
            );

            // bob should not match anything as the secret hasn't been committed yet
            assert_eq!(chain.valid_cap_grant(&function, &bob, secret)?, None);
        }

        {
            let mut chain = SourceChain::new(env.clone().into())?;
            chain.put_cap_grant(grant.clone()).await?;

            // ideally the following would work, but it won't because currently
            // we can't get grants from the scratch space
            // this will be fixed once we add the capability index

            // assert_eq!(
            //     chain.get_persisted_cap_grant_by_secret(secret)?,
            //     Some(grant.clone().into())
            // );

            env.guard()
                .with_commit(|writer| chain.flush_to_txn(writer))?;
        }

        {
            let chain = SourceChain::new(env.clone().into())?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, secret)?,
                Some(CapGrant::Authorship(alice)),
            );

            // bob should be granted with the committed grant as it matches the secret he passes to
            // alice at runtime
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, secret)?,
                Some(grant.into())
            );
        }

        Ok(())
    }

    // @todo bring all this back when we want to administer cap claims better
    // #[tokio::test(threaded_scheduler)]
    // async fn test_get_cap_claim() -> SourceChainResult<()> {
    //     let test_env = test_cell_env();
    //     let env = test_env.env();
    //     let env = env.guard().await;
    //     let secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
    //     let agent_pubkey = fake_agent_pubkey_1().into();
    //     let claim = CapClaim::new("tag".into(), agent_pubkey, secret.clone());
    //     {
    //         let mut store = SourceChainBuf::new(env.clone().into(), &env).await?;
    //         store
    //             .genesis(fake_dna_hash(1), fake_agent_pubkey_1(), None)
    //             .await?;
    //         env.with_commit(|writer| store.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let mut chain = SourceChain::new(env.clone().into(), &env).await?;
    //         chain.put_cap_claim(claim.clone()).await?;
    //
    // // ideally the following would work, but it won't because currently
    // // we can't get claims from the scratch space
    // // this will be fixed once we add the capability index
    //
    // // assert_eq!(
    // //     chain.get_persisted_cap_claim_by_secret(&secret)?,
    // //     Some(claim.clone())
    // // );
    //
    //         env.with_commit(|writer| chain.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let chain = SourceChain::new(env.clone().into(), &env).await?;
    //         assert_eq!(
    //             chain.get_persisted_cap_claim_by_secret(&secret).await?,
    //             Some(claim)
    //         );
    //     }
    //
    //     Ok(())
    // }
}
