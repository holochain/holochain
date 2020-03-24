use holochain_2020::core::state::{
    cascade::Cascade, chain_cas::ChainCasBuf, chain_meta::ChainMetaBuf,
};
use sx_state::{
    buffer::{BufferedStore, KvBuf},
    db::PRIMARY_CHAIN_ENTRIES,
    env::{EnvironmentRef, ReadManager, WriteManager},
    error::{DatabaseError, DatabaseResult},
    exports::SingleStore,
    test_utils::test_env,
};
use sx_types::{
    agent::AgentId,
    entry::Entry,
    persistence::cas::content::{Address, AddressableContent},
};

/// Makeshift commit
fn commit(env: &EnvironmentRef, primary: SingleStore, entry: Entry) -> DatabaseResult<Address> {
    let address = entry.address();

    let writer = env.with_reader::<DatabaseError, _, _>(|reader| {
        let mut writer = env.writer()?;
        let mut kv: KvBuf<Address, Entry> = KvBuf::new(&reader, primary)?;

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
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;

    let primary_entries_cas = dbs.get(&*PRIMARY_CHAIN_ENTRIES)?;

    // TODO create a cache and a cas for store and meta
    let primary = ChainCasBuf::primary(&reader, &dbs)?;
    let primary_meta = ChainMetaBuf::primary(&reader, &dbs)?;

    let cache = ChainCasBuf::cache(&reader, &dbs)?;
    let cache_meta = ChainMetaBuf::cache(&reader, &dbs)?;

    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    let address = commit(&env_ref, *primary_entries_cas, jimbo.clone())?;

    // TODO Pass in stores as references
    // TODO How will we create a struct with references? Maybe it should create from
    // the stores and must only live as long as them.
    let cascade = Cascade::new(&primary, &primary_meta, &cache, &cache_meta);
    let entry = cascade.dht_get(address).await;
    assert_eq!(entry, jimbo);
    Ok(())
}
