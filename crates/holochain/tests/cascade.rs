use fixt::prelude::*;
use holochain::core::state::{
    cascade::Cascade,
    metadata::{LinkMetaKey, MetadataBuf},
    source_chain::{SourceChainBuf, SourceChainResult},
};
use holochain::{test_utils::test_network, fixt::ZomeIdFixturator};
use holochain_state::{env::ReadManager, test_utils::test_cell_env};
use holochain_types::{
    entry::EntryHashed,
    header,
    prelude::*,
    test_utils::{fake_agent_pubkey_1, fake_agent_pubkey_2, fake_header_hash},
    Header,
};
use holochain_zome_types::entry::Entry;
use holochain_zome_types::link::LinkTag;

fn fixtures() -> (
    AgentPubKey,
    Header,
    EntryHashed,
    AgentPubKey,
    Header,
    EntryHashed,
) {
    let previous_header = fake_header_hash("previous");

    let jimbo_id = fake_agent_pubkey_1();
    let jessy_id = fake_agent_pubkey_2();

    let (jimbo_entry, jessy_entry) = tokio_safe_block_on::tokio_safe_block_on(
        async {
            (
                EntryHashed::with_data(Entry::Agent(jimbo_id.clone().into()))
                    .await
                    .unwrap(),
                EntryHashed::with_data(Entry::Agent(jessy_id.clone().into()))
                    .await
                    .unwrap(),
            )
        },
        std::time::Duration::from_secs(1),
    )
    .unwrap();

    let jimbo_header = Header::EntryCreate(header::EntryCreate {
        author: jimbo_id.clone(),
        timestamp: Timestamp::now(),
        header_seq: 0,
        prev_header: previous_header.clone().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_hash: jimbo_entry.as_hash().clone(),
    });

    let jessy_header = Header::EntryCreate(header::EntryCreate {
        author: jessy_id.clone(),
        timestamp: Timestamp::now(),
        header_seq: 0,
        prev_header: previous_header.clone().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_hash: jessy_entry.as_hash().clone(),
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
    let primary_meta = MetadataBuf::primary(&reader, &dbs)?;
    let cache_meta = MetadataBuf::cache(&reader, &dbs)?;

    let (_jimbo_id, jimbo_header, jimbo_entry, _jessy_id, jessy_header, jessy_entry) = fixtures();

    let base = jimbo_entry.as_hash().clone();
    source_chain
        .put_raw(jimbo_header, Some(jimbo_entry.as_content().clone()))
        .await?;
    source_chain
        .put_raw(jessy_header, Some(jessy_entry.as_content().clone()))
        .await?;

    let (_n, _r, cell_network) = test_network().await;

    // Pass in stores as references
    let cascade = Cascade::new(
        &source_chain.cas(),
        &primary_meta,
        &cache.cas(),
        &cache_meta,
        cell_network,
    );
    let tag = LinkTag::new(BytesFixturator::new(Unpredictable).next().unwrap());
    let zome_id = ZomeIdFixturator::new(Unpredictable).next().unwrap();
    let key = LinkMetaKey::BaseZomeTag(&base, zome_id, &tag);

    let links = cascade.dht_get_links(&key).await?;
    let link = links.into_iter().next();
    assert_eq!(link, None);
    Ok(())
}
