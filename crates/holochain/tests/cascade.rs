use holo_hash::EntryHash;
use holochain_2020::core::state::{
    cascade::Cascade, chain_meta::ChainMetaBuf, source_chain::SourceChainBuf,
};
use std::convert::TryInto;
use sx_state::{env::ReadManager, error::DatabaseResult, test_utils::test_cell_env};
use sx_types::{entry::Entry, test_utils::fake_agent_hash};

#[tokio::test]
async fn get_links() -> DatabaseResult<()> {
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;

    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;

    // create a cache and a cas for store and meta
    let primary_meta = ChainMetaBuf::primary(&reader, &dbs)?;
    let cache_meta = ChainMetaBuf::cache(&reader, &dbs)?;

    let jimbo_id = fake_agent_hash("Jimbo");
    let jimbo = Entry::AgentKey(jimbo_id.clone());
    let jessy_id = fake_agent_hash("Jessy");
    let jessy = Entry::AgentKey(jessy_id.clone());
    let base: EntryHash = (&jimbo).try_into()?;
    source_chain.put_entry(jimbo, &jimbo_id)?;
    source_chain.put_entry(jessy, &jessy_id)?;

    // Pass in stores as references
    let cascade = Cascade::new(
        &source_chain.cas(),
        &primary_meta,
        &cache.cas(),
        &cache_meta,
    );
    let links = cascade.dht_get_links(base.into(), "").await?;
    let link = links.into_iter().next();
    assert_eq!(link, None);
    Ok(())
}
