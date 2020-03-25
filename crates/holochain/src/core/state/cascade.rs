//! get vs get_links
//! default vs options
//! fast vs strict #set by app dev
//!
//! get Default - Get's the latest version
//! Scratch if Live -> Return
//! Scratch if Dead -> None
//! Scratch NotFound -> Goto Cas
//! Cas Live -> Return
//! Cas NotFound -> Goto cache
//! Cas _ -> None
//! Cache Live -> Return
//! Cache Pending -> Goto Network
//! Cache NotFound -> Goto Network
//! Cache _ -> None
//!
//! get_links Default - Get's the latest version
//! Always try authority
//! if I'm an authority
//! Scratch Found-> Return
//! Scratch NotFound -> Goto Cas
//! Cas Found -> Return
//! Cas NotFound -> Goto Network
//! else
//! Network Found -> Return
//! Network NotFound -> Goto Cache
//! Cache Found -> Return
//! Cache NotFound -> None
//!
//! gets most recent N links with default N (50)
//! Page number
//! load_true loads the results into cache

use super::{
    chain_cas::ChainCasBuf,
    chain_meta::{ChainMetaBufT, Crud},
};
use std::collections::HashSet;
use sx_state::{error::DatabaseResult, prelude::Reader};
use sx_types::{entry::Entry, persistence::cas::content::Address};
use tracing::*;

/// TODO Network is not handled here, must either return
/// the fact that a network get is required or take a reference to the network.
#[allow(dead_code)]
pub struct Cascade<'env, C>
where
    C: ChainMetaBufT<'env>,
{
    primary: &'env ChainCasBuf<'env, Reader<'env>>,
    //primary_meta: &'env ChainMetaBuf<'env, ()>,
    primary_meta: &'env C,

    cache: &'env ChainCasBuf<'env, Reader<'env>>,
    cache_meta: &'env C,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env, C> Cascade<'env, C>
where
    C: ChainMetaBufT<'env>,
{
    /// Take references to cas and cache
    pub fn new(
        primary: &'env ChainCasBuf<'env, Reader<'env>>,
        //primary_meta: &'env ChainMetaBuf<'env, ()>,
        primary_meta: &'env C,
        cache: &'env ChainCasBuf<'env, Reader<'env>>,
        cache_meta: &'env C,
    ) -> Self {
        Cascade {
            primary,
            primary_meta,
            cache,
            cache_meta,
        }
    }
    #[instrument(skip(self))]
    pub async fn dht_get(&self, address: Address) -> DatabaseResult<Option<Entry>> {
        let entry = self
            .primary
            .get_entry(&address)?
            .and_then(|entry| {
                self.primary_meta
                    .get_crud(&address)
                    .ok()
                    .map(|crud| (crud, entry))
            })
            .filter(|(crud, _)| if let Crud::Live = crud { true } else { false })
            .map(|crud_entry| {
                trace!(?crud_entry);
                crud_entry
            })
            .map(|(_, entry)| entry);
        trace!(?entry);
        Ok(entry)
    }
    pub async fn dht_get_links(
        &self,
        base: Address,
        tag: String,
    ) -> DatabaseResult<HashSet<Address>> {
        self.primary_meta.get_links(&base, tag)
    }
}

#[cfg(test)]
mod test {
    use super::Cascade;
    use crate::core::state::{
        chain_meta::{Crud, MockChainMetaBuf},
        source_chain::SourceChainBuf,
    };
    use mockall::*;
    use sx_state::{env::ReadManager, error::DatabaseResult, test_utils::test_env};
    use sx_types::{agent::AgentId, entry::Entry, observability, prelude::AddressableContent};
    #[tokio::test]
    async fn live_local_return() -> DatabaseResult<()> {
        // setup some data thats in the scratch
        let env = test_env();
        let dbs = env.dbs().await?;
        let env_ref = env.guard().await;
        let reader = env_ref.reader()?;
        let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
        let cache = SourceChainBuf::cache(&reader, &dbs)?;
        let jimbo_id = AgentId::generate_fake("jimbos_id");
        let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
        source_chain.put_entry(jimbo.clone(), &jimbo_id);
        let address = jimbo.address();

        // set it's metadata to LIVE
        let mut mock_primary_meta = MockChainMetaBuf::new();
        let mock_cache_meta = MockChainMetaBuf::new();
        mock_primary_meta
            .expect_get_crud()
            .with(predicate::eq(address.clone()))
            .returning(|_| Ok(Crud::Live));

        // call dht_get with above address
        let cascade = Cascade::new(
            &source_chain.cas(),
            &mock_primary_meta,
            &cache.cas(),
            &mock_cache_meta,
        );
        let entry = cascade.dht_get(address).await?;
        // check it returns
        assert_eq!(entry, Some(jimbo));
        // check it doesn't hit the cache
        // this is implied by the mock not expecting calls
        // TODO check it doesn't ask the network
        Ok(())
    }

    #[tokio::test]
    async fn dead_local_none() -> DatabaseResult<()> {
        observability::test_run().ok();
        // setup some data thats in the scratch
        let env = test_env();
        let dbs = env.dbs().await?;
        let env_ref = env.guard().await;
        let reader = env_ref.reader()?;
        let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
        let cache = SourceChainBuf::cache(&reader, &dbs)?;
        let jimbo_id = AgentId::generate_fake("jimbos_id");
        let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
        source_chain.put_entry(jimbo.clone(), &jimbo_id);
        let address = jimbo.address();

        // set it's metadata to Dead
        let mut mock_primary_meta = MockChainMetaBuf::new();
        let mock_cache_meta = MockChainMetaBuf::new();
        mock_primary_meta
            .expect_get_crud()
            .with(predicate::eq(address.clone()))
            .returning(|_| Ok(Crud::Dead));

        // call dht_get with above address
        let cascade = Cascade::new(
            &source_chain.cas(),
            &mock_primary_meta,
            &cache.cas(),
            &mock_cache_meta,
        );
        let entry = cascade.dht_get(address).await?;
        // check it returns
        assert_eq!(entry, None);
        // check it doesn't hit the cache
        // this is implied by the mock not expecting calls
        // TODO check it doesn't ask the network
        Ok(())
    }
}
