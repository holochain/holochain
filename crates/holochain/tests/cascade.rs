use holochain_2020::core::state::cascade::Cascade;
use rkv::StoreOptions;
use sx_state::{
    buffer::{BufferedStore, KvBuf},
    env::{EnvironmentRef, ReadManager, WriteManager},
    error::{DatabaseError, DatabaseResult},
    test_utils::test_env,
};
use sx_types::{
    agent::AgentId,
    entry::Entry,
    persistence::cas::content::{Address, AddressableContent},
};

/// Makeshift commit
fn commit(env: &EnvironmentRef, entry: Entry) -> DatabaseResult<Address> {
    let db = env.inner().open_single("kv", StoreOptions::create())?;
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

    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    let address = commit(&env, jimbo.clone())?;

    let cascade = Cascade::new();
    let entry = cascade.dht_get(address).await;
    assert_eq!(entry, jimbo);
    Ok(())
}
