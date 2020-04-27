use holo_hash::EntryHash;
use holochain_2020::core::state::{
    cascade::Cascade, chain_meta::ChainMetaBuf, source_chain::SourceChainBuf,
};
use holochain_state::{env::ReadManager, error::DatabaseResult, test_utils::test_cell_env};
use holochain_types::{
    entry::Entry,
    signature::Signature,
    test_utils::{fake_agent_hash, fake_header_hash},
};
use std::convert::TryInto;

fn fixtures() -> (AgentHash, ChainElement, AgentHash, ChainElement) {
    let previous_header = fake_header_hash("previous");

    let jimbo_id = fake_agent_hash("Jimbo");
    let jimbo = Entry::AgentKey(jimbo_id.clone());
    let jessy_id = fake_agent_hash("Jessy");
    let jessy = Entry::AgentKey(jessy_id.clone());

    let jimbo_header = ChainHeader::EntryCreate(header::EntryCreate {
        timestamp: chrono::Utc::now().timestamp().into(),
        author: jimbo_id.clone(),
        prev_headr: previous_header,
        entry_type: header::EntryType::AgentKey,
        entry_address: jimbo.entry_address(),
    });
    let jimbo_element = ChainElement(Signature::fake(), jimbo_header, Some(jimbo));

    let jessy_header = ChainHeader::EntryCreate(header::EntryCreate {
        timestamp: chrono::Utc::now().timestamp().into(),
        author: jessy_id.clone(),
        prev_headr: previous_header,
        entry_type: header::EntryType::AgentKey,
        entry_address: jessy.entry_address(),
    });
    let jessy_element = ChainElement(Signature::fake(), jessy_header, Some(jessy));
    (jimbo_id, jimbo_element, jessy_id, jessy_element)
}

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

    let (jimbo_id, jimbo, jessy_id, jesse) = fixutres();

    let base: EntryHash = (&jimbo).try_into()?;
    source_chain.put_element(jimbo)?;
    source_chain.put_element(jessy)?;

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
