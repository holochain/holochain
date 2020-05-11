use holochain_2020::core::state::{
    cascade::Cascade,
    chain_meta::ChainMetaBuf,
    source_chain::{SourceChainBuf, SourceChainResult},
};
use holochain_state::{env::ReadManager, test_utils::test_cell_env};
use holochain_types::{
    address::EntryAddress,
    chain_header::ChainHeader,
    header,
    prelude::*,
    test_utils::{fake_agent_pubkey_1, fake_agent_pubkey_2, fake_header_hash},
    Header,
};
use holochain_zome_types::entry::Entry;

fn fixtures() -> (AgentPubKey, Header, Entry, AgentPubKey, Header, Entry) {
    let previous_header = fake_header_hash("previous");

    let jimbo_id = fake_agent_pubkey_1();
    let jimbo_entry = Entry::Agent(jimbo_id.clone().into());
    let jessy_id = fake_agent_pubkey_2();
    let jessy_entry = Entry::Agent(jessy_id.clone().into());

    let jimbo_header = Header::EntryCreate(header::EntryCreate {
        timestamp: Timestamp::now(),
        author: jimbo_id.clone(),
        prev_header: previous_header.clone().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_address: (&jimbo_entry).try_into().unwrap(),
    });

    let jessy_header = Header::EntryCreate(header::EntryCreate {
        timestamp: Timestamp::now(),
        author: jessy_id.clone(),
        prev_header: previous_header.clone().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_address: (&jessy_entry).try_into().unwrap(),
    });
    (
        jimbo_id,
        jimbo_header,
        jimbo_entry,
        jessy_id,
        jessy_header,
        jessy_entry,
    )
}

#[tokio::test(threaded_scheduler)]
async fn get_links() -> SourceChainResult<()> {
    let env = test_cell_env();
    let dbs = env.dbs().await;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;

    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;

    // create a cache and a cas for store and meta
    let primary_meta = ChainMetaBuf::primary(&reader, &dbs)?;
    let cache_meta = ChainMetaBuf::cache(&reader, &dbs)?;

    let (_jimbo_id, jimbo_header, jimbo_entry, _jessy_id, jessy_header, jessy_entry) = fixtures();

    let base = EntryAddress::try_from(&jimbo_entry)?;
    source_chain.put(jimbo_header, Some(jimbo_entry)).await?;
    source_chain.put(jessy_header, Some(jessy_entry)).await?;

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
