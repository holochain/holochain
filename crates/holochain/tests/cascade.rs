use holochain_2020::core::{
    net::MockNetRequester,
    state::{cascade::Cascade, chain_meta::ChainMetaBuf, source_chain::SourceChainBuf},
};
use std::collections::HashSet;
use sx_state::{env::ReadManager, error::DatabaseResult, test_utils::test_env};
use sx_types::{agent::AgentId, entry::Entry, persistence::cas::content::AddressableContent};

#[tokio::test]
async fn get_links() -> DatabaseResult<()> {
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;

    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;

    // create a cache and a cas for store and meta
    //let mut primary_meta = MockChainMetaBuf::primary(&reader, &dbs)?;
    let primary_meta = ChainMetaBuf::primary(&reader, &dbs)?;

    //let cache_meta = MockChainMetaBuf::cache(&reader, &dbs)?;
    let cache_meta = ChainMetaBuf::cache(&reader, &dbs)?;

    let jimbo_id = AgentId::generate_fake("Jimbo");
    let jimbo = Entry::AgentId(jimbo_id.clone());
    let address = jimbo.address();
    // TODO use a source chain buffer instead of adding a manual commit
    source_chain.put_entry(jimbo, &jimbo_id);

    let mock_network = MockNetRequester::new();
    // Pass in stores as references
    let cascade = Cascade::new(
        &source_chain.cas(),
        &primary_meta,
        &cache.cas(),
        &cache_meta,
        mock_network,
    );
    let links = cascade.dht_get_links(address, "").await;
    assert_eq!(links, Ok(HashSet::new()));
    Ok(())
}
