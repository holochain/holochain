//! A SourceChain is guaranteed to be initialized, i.e. it has gone through the CellGenesis workflow.
//! It has the same interface as its underlying SourceChainBuf, except that certain operations,
//! which would return Option in the SourceChainBuf, like getting the source chain head, or the AgentPubKey,
//! cannot fail, so the function return types reflect that.

pub use error::*;
use fallible_iterator::FallibleIterator;
use holo_hash::*;
use holochain_state::{
    buffer::BufferedStore, db::GetDb, error::DatabaseResult, fresh_reader, prelude::*,
};
use holochain_types::{prelude::*, EntryHashed};
use holochain_zome_types::capability::CapAccess;
use holochain_zome_types::capability::GrantedFunction;
use holochain_zome_types::{
    capability::{CapGrant, CapSecret},
    entry::{CapClaimEntry, Entry},
    header::{builder, EntryType, Header, HeaderBuilder, HeaderBuilderCommon, HeaderInner},
};
use shrinkwraprs::Shrinkwrap;
pub use source_chain_buffer::*;
use std::collections::HashSet;

mod error;
mod source_chain_buffer;

/// A wrapper around [SourceChainBuf] with the assumption that the source chain has been initialized,
/// i.e. has undergone Genesis.
#[derive(Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct SourceChain(pub SourceChainBuf);

impl SourceChain {
    pub async fn agent_pubkey(&self) -> SourceChainResult<AgentPubKey> {
        self.0
            .agent_pubkey()
            .await?
            .ok_or(SourceChainError::InvalidStructure(
                ChainInvalidReason::GenesisDataMissing,
            ))
    }

    pub fn chain_head(&self) -> SourceChainResult<&HeaderHash> {
        self.0.chain_head().ok_or(SourceChainError::ChainEmpty)
    }

    pub async fn new(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::new(env, dbs).await?.into())
    }

    pub async fn public_only(env: EnvironmentRead, dbs: &impl GetDb) -> DatabaseResult<Self> {
        Ok(SourceChainBuf::public_only(env, dbs).await?.into())
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
            author: self.agent_pubkey().await?,
            timestamp: Timestamp::now().into(),
            header_seq: self.len() as u32,
            prev_header: self.chain_head()?.to_owned(),
        };
        let header = header_builder.build(common).into();
        self.put_raw(header, maybe_entry).await
    }

    /// Add a CapClaimEntry to the source chain
    pub async fn put_cap_claim(
        &mut self,
        claim_entry: CapClaimEntry,
    ) -> SourceChainResult<HeaderHash> {
        let (entry, entry_hash) = EntryHashed::from_content(Entry::CapClaim(claim_entry))
            .await
            .into_inner();
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
    pub async fn valid_cap_grant(
        &self,
        check_function: &GrantedFunction,
        check_agent: &AgentPubKey,
        check_secret: &CapSecret,
    ) -> SourceChainResult<Option<CapGrant>> {
        let author_grant = CapGrant::from(self.agent_pubkey().await?);
        if author_grant.is_valid(check_function, check_agent, check_secret) {
            return Ok(Some(author_grant));
        }

        let committed_valid_grant = fresh_reader!(self.env(), |r| {
            let (references, headers): (
                HashSet<HeaderHash>,
                Vec<HoloHashed<holochain_zome_types::element::SignedHeader>>,
            ) = self
                .0
                .headers()
                .iter_fail(&r)?
                .filter(|header| {
                    Ok(match header.as_content().header() {
                        Header::EntryCreate(create) => match create.entry_type {
                            EntryType::CapGrant => true,
                            EntryType::AgentPubKey => true,
                            _ => false,
                        },
                        Header::EntryUpdate(update) => match update.entry_type {
                            EntryType::CapGrant => true,
                            EntryType::AgentPubKey => true,
                            _ => false,
                        },
                        Header::ElementDelete(_) => true,
                        _ => false,
                    })
                })
                // extract all the update/delete references into a hashmap
                .fold(
                    (HashSet::new(), vec![]),
                    |(mut references, mut headers), header| {
                        match header.as_content().header() {
                            Header::EntryUpdate(update) => {
                                references.insert(update.original_header_address.clone());
                            }
                            Header::ElementDelete(delete) => {
                                references.insert(delete.removes_address.clone());
                            }
                            _ => {}
                        }
                        // this is a best-effort attempt to avoid putting things we already know as
                        // referenced into the returned vec
                        // it isn't comprehensive because it relies on ordering but it's an easy
                        // and relatively safe optimisation to not do further processing here
                        if !references.contains(header.as_hash()) {
                            headers.push(header);
                        }

                        Ok((references, headers))
                    },
                )?;

            // second pass over the headers to make sure that all referenced headers are removed
            // this makes the process reliable even if the iterators don't follow the chain order
            let live_cap_grants: HashSet<_> = headers
                .iter()
                .filter(|header| !references.contains(header.as_hash()))
                .filter_map(|header| match header.as_content().header() {
                    Header::EntryCreate(create) => Some(create.entry_hash.clone()),
                    Header::EntryUpdate(update) => Some(update.entry_hash.clone()),
                    _ => None,
                })
                .collect();

            self
            .0
            .elements()
            .private_entries()
            .expect(
                "SourceChainBuf must have access to private entries in order to access CapGrants",
            )
            .iter_fail(&r)?
            .filter(|entry| {
                Ok(live_cap_grants.contains(entry.as_hash()))
            })
            // filter all entries down to only cap grant entries
            .filter_map(|entry| {
                Ok(entry.as_cap_grant())
            })
            // filter down to only the grants for this function
            .filter(|grant| {
                Ok(grant.is_valid(check_function, check_agent, check_secret))
            })
            // if there are still multiple grants, fold them down based on specificity
            // authorship > assigned > transferable > unrestricted
            .fold(None, |mut acc, grant| {
                acc = match &grant {
                    CapGrant::Authorship(_) => Some(grant),
                    CapGrant::ZomeCall(zome_call_cap_grant) => {
                        match &zome_call_cap_grant.access {
                            CapAccess::Assigned { .. } => match &acc {
                                // authorship acc takes precedence
                                Some(CapGrant::Authorship(_)) => acc,
                                Some(CapGrant::ZomeCall(acc_zome_call_cap_grant)) => {
                                    match acc_zome_call_cap_grant.access {
                                        // an assigned acc takes precedence
                                        CapAccess::Assigned { .. } => acc,
                                        // current grant takes precedence over all other accs
                                        _ => Some(grant),
                                    }
                                }
                                None => Some(grant),
                            },
                            CapAccess::Transferable { .. } => match &acc {
                                // authorship acc takes precedence
                                Some(CapGrant::Authorship(_)) => acc,
                                Some(CapGrant::ZomeCall(acc_zome_call_cap_grant)) => {
                                    match acc_zome_call_cap_grant.access {
                                        // an assigned acc takes precedence
                                        CapAccess::Assigned { .. } => acc,
                                        // transferable acc takes precedence
                                        CapAccess::Transferable { .. } => acc,
                                        // current grant takes preference over other accs
                                        _ => Some(grant),
                                    }
                                }
                                None => Some(grant),
                            }
                            CapAccess::Unrestricted => match acc {
                                Some(_) => acc,
                                None => Some(grant),
                            }
                        }
                    }
                };
                Ok(acc)
            })
        })?;
        Ok(committed_valid_grant)
    }

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
}

impl From<SourceChainBuf> for SourceChain {
    fn from(buffer: SourceChainBuf) -> Self {
        Self(buffer)
    }
}

impl BufferedStore for SourceChain {
    type Error = SourceChainError;

    fn flush_to_txn(self, writer: &mut Writer) -> Result<(), Self::Error> {
        self.0.flush_to_txn(writer)?;
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
    use std::collections::HashSet;

    #[tokio::test(threaded_scheduler)]
    async fn test_get_cap_grant() -> SourceChainResult<()> {
        let test_env = test_cell_env();
        let arc = test_env.env();
        let env = arc.guard().await;
        let secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
        let access = CapAccess::from(secret.clone());
        // @todo curry
        let _curry = CurryPayloadsFixturator::new(Empty).next().unwrap();
        let function: GrantedFunction = ("foo".into(), "bar".into());
        let mut functions: GrantedFunctions = HashSet::new();
        functions.insert(function.clone());
        let grant = ZomeCallCapGrant::new("tag".into(), access.clone(), functions.clone());
        let mut agents = AgentPubKeyFixturator::new(Predictable);
        let alice = agents.next().unwrap();
        let bob = agents.next().unwrap();
        {
            let mut store = SourceChainBuf::new(arc.clone().into(), &env).await?;
            store.genesis(fake_dna_hash(1), alice.clone(), None).await?;
            env.with_commit(|writer| store.flush_to_txn(writer))?;
        }

        {
            let chain = SourceChain::new(arc.clone().into(), &env).await?;
            // alice should always find her authorship even if no grants have been committed
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, &secret).await?,
                Some(CapGrant::Authorship(alice.clone())),
            );

            // bob should not match anything as the secret hasn't been committed yet
            assert_eq!(chain.valid_cap_grant(&function, &bob, &secret).await?, None);
        }

        let (original_header_address, original_entry_address) = {
            let mut chain = SourceChain::new(arc.clone().into(), &env).await?;
            let (entry, entry_hash) = EntryHashed::from_content(Entry::CapGrant(grant.clone()))
                .await
                .into_inner();
            let header_builder = builder::EntryCreate {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
            };
            let header = chain.put(header_builder, Some(entry)).await?;

            env.with_commit(|writer| chain.flush_to_txn(writer))?;

            (header, entry_hash)
        };

        {
            let chain = SourceChain::new(arc.clone().into(), &env).await?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, &secret).await?,
                Some(CapGrant::Authorship(alice.clone())),
            );

            // bob should be granted with the committed grant as it matches the secret he passes to
            // alice at runtime
            assert_eq!(
                chain.valid_cap_grant(&function, &bob, &secret).await?,
                Some(grant.clone().into())
            );
        }

        // let's roll the secret and assign the grant to bob specifically
        let mut assignees = HashSet::new();
        assignees.insert(bob.clone());
        let updated_secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
        let updated_access = CapAccess::from((updated_secret.clone(), assignees));
        let updated_grant = ZomeCallCapGrant::new("tag".into(), updated_access.clone(), functions);

        let (updated_header_hash, updated_entry_hash) = {
            let mut chain = SourceChain::new(arc.clone().into(), &env).await?;
            let (entry, entry_hash) =
                EntryHashed::from_content(Entry::CapGrant(updated_grant.clone()))
                    .await
                    .into_inner();
            let header_builder = builder::EntryUpdate {
                entry_type: EntryType::CapGrant,
                entry_hash: entry_hash.clone(),
                original_header_address,
                original_entry_address,
            };
            let header = chain.put(header_builder, Some(entry)).await?;

            env.with_commit(|writer| chain.flush_to_txn(writer))?;

            (header, entry_hash)
        };

        {
            let chain = SourceChain::new(arc.clone().into(), &env).await?;
            // alice should find her own authorship with higher priority than the committed grant
            // even if she passes in the secret
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, &secret).await?,
                Some(CapGrant::Authorship(alice.clone())),
            );
            assert_eq!(
                chain
                    .valid_cap_grant(&function, &alice, &updated_secret)
                    .await?,
                Some(CapGrant::Authorship(alice.clone())),
            );

            // bob MUST provide the updated secret as the old one is invalidated by the new one
            assert_eq!(chain.valid_cap_grant(&function, &bob, &secret).await?, None);
            assert_eq!(
                chain
                    .valid_cap_grant(&function, &bob, &updated_secret)
                    .await?,
                Some(updated_grant.into())
            );
        }

        {
            let mut chain = SourceChain::new(arc.clone().into(), &env).await?;
            let header_builder = builder::ElementDelete {
                removes_address: updated_header_hash,
                removes_entry_address: updated_entry_hash,
            };
            chain.put(header_builder, None).await?;

            env.with_commit(|writer| chain.flush_to_txn(writer))?;
        }

        {
            let chain = SourceChain::new(arc.clone().into(), &env).await?;
            // alice should find her own authorship
            assert_eq!(
                chain.valid_cap_grant(&function, &alice, &secret).await?,
                Some(CapGrant::Authorship(alice.clone())),
            );
            assert_eq!(
                chain
                    .valid_cap_grant(&function, &alice, &updated_secret)
                    .await?,
                Some(CapGrant::Authorship(alice)),
            );

            // bob has no access
            assert_eq!(chain.valid_cap_grant(&function, &bob, &secret).await?, None);
            assert_eq!(
                chain
                    .valid_cap_grant(&function, &bob, &updated_secret)
                    .await?,
                None
            );
        }

        Ok(())
    }

    // #[tokio::test(threaded_scheduler)]
    // async fn test_get_cap_claim() -> SourceChainResult<()> {
    //     let test_env = test_cell_env();
    //     let arc = test_env.env();
    //     let env = arc.guard().await;
    //     let secret = CapSecretFixturator::new(Unpredictable).next().unwrap();
    //     let agent_pubkey = fake_agent_pubkey_1().into();
    //     let claim = CapClaim::new("tag".into(), agent_pubkey, secret.clone());
    //     {
    //         let mut store = SourceChainBuf::new(arc.clone().into(), &env).await?;
    //         store
    //             .genesis(fake_dna_hash(1), fake_agent_pubkey_1(), None)
    //             .await?;
    //         env.with_commit(|writer| store.flush_to_txn(writer))?;
    //     }
    //
    //     {
    //         let mut chain = SourceChain::new(arc.clone().into(), &env).await?;
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
    //         let chain = SourceChain::new(arc.clone().into(), &env).await?;
    //         assert_eq!(
    //             chain.get_persisted_cap_claim_by_secret(&secret).await?,
    //             Some(claim)
    //         );
    //     }
    //
    //     Ok(())
    // }
}
