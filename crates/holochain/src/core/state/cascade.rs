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

use super::chain_cas::ChainCasBuf;
use sx_state::{buffer::KvvBuf, prelude::Reader};
use sx_types::{entry::Entry, persistence::cas::content::Address};

/// TODO Network is not handled here, must either return
/// the fact that a network get is required or take a reference to the network.
#[allow(dead_code)]
pub struct Cascade<'env> {
    cas: &'env ChainCasBuf<'env, Reader<'env>>,
    cas_meta: &'env KvvBuf<'env, Address, ()>,

    cache: &'env ChainCasBuf<'env, Reader<'env>>,
    cache_meta: &'env KvvBuf<'env, Address, ()>,
}

/// Should these functions be sync or async?
/// Depends on how much computation, and if writes are involved
impl<'env> Cascade<'env> {
    /// Take references to cas and cache
    pub fn new(
        cas: &'env ChainCasBuf<'env, Reader<'env>>,
        cas_meta: &'env KvvBuf<'env, Address, ()>,

        cache: &'env ChainCasBuf<'env, Reader<'env>>,
        cache_meta: &'env KvvBuf<'env, Address, ()>,
    ) -> Self {
        Cascade {
            cas,
            cas_meta,
            cache,
            cache_meta,
        }
    }
    pub async fn dht_get(&self, address: Address) -> Entry {
        unimplemented!()
    }
}

#[cfg(test)]
mod test {
    use crate::core::state::cascade::Cascade;
    use crate::core::state::chain_cas::ChainCasBuf;
    use rkv::StoreOptions;
    use sx_state::{
        buffer::{BufferedStore, KvBuf},
        env::{EnvironmentRef, ReadManager, WriteManager},
        error::{DatabaseError, DatabaseResult},
        test_utils::test_env,
        exports::SingleStore,
    };
    use sx_types::{
        agent::AgentId,
        entry::Entry,
        persistence::cas::content::{Address, AddressableContent},
    };

    /// Makeshift commit
    fn commit(env: EnvironmentRef, db: &SingleStore, entry: Entry) -> DatabaseResult<Address> {
        let address = entry.address();

        let writer = env.with_reader::<DatabaseError, _, _>(|reader| {
            let mut writer = env.writer()?;
            let mut kv: KvBuf<Address, Entry> = KvBuf::new(&reader, db)?;

            kv.put(address.clone(), entry);
            kv.flush_to_txn(&mut writer)?;

            Ok(writer)
        })?;

        // Finish finalizing the transaction
        writer.commit()?;
        Ok(address)
    }

    #[tokio::test]
    async fn get() -> DatabaseResult<()> {
        let arc = test_env();
        let env = arc.guard().await;
        let cas = env.inner().open_single("cas", StoreOptions::create())?;
        let cas_headers = env.inner().open_single("cas_headers", StoreOptions::create())?;
        let cas_meta = env.inner().open_multi("cas_meta", StoreOptions::create())?;
        let cache = env.inner().open_single("cache", StoreOptions::create())?;
        let cache_cache  = env.inner().open_single("cache_headers", StoreOptions::create())?;
        let cache_meta = env.inner().open_multi("cache_meta", StoreOptions::create())?;

        let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
        let address = commit(env,&cas, jimbo.clone())?;

        env.with_reader::<DatabaseError, _, _>(|reader| {
            let cas = ChainCasBuf::new(reader, cas, cas_headers).expect("Failed to make cas");
            let cache = ChainCasBuf::new(reader, cache, cache_headers).expect("Failed to make cache");
            Ok(())
        })?;

        // TODO create a cache and a cas for store and meta

        // TODO Pass in stores as references
        // TODO How will we create a struct with references? Maybe it should create from
        // the stores and must only live as long as them.
        let cascade = Cascade::new(&cas, &cas_meta, &cache, &cache_meta);
        let entry = cascade.dht_get(address).await;
        assert_eq!(entry, jimbo);
        Ok(())
    }
}
