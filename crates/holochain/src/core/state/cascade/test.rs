use super::Cascade;
use crate::core::state::{
    chain_meta::{EntryDhtStatus, MockChainMetaBuf},
    source_chain::{SourceChainBuf, SourceChainResult},
};
use holochain_state::{
    db::DbManager, env::ReadManager, error::DatabaseResult, prelude::Reader,
    test_utils::test_cell_env,
};
use holochain_types::{
    address::EntryAddress,
    chain_header::ChainHeader,
    entry::Entry,
    header, observability,
    prelude::*,
    test_utils::{fake_agent_pubkey, fake_header_hash},
};
use maplit::hashset;
use mockall::*;
use std::collections::HashSet;

#[allow(dead_code)]
struct Chains<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
    cache: SourceChainBuf<'env, Reader<'env>>,
    jimbo_id: AgentPubKey,
    jimbo_header: ChainHeader,
    jimbo_entry: Entry,
    jessy_id: AgentPubKey,
    jessy_header: ChainHeader,
    jessy_entry: Entry,
    mock_primary_meta: MockChainMetaBuf,
    mock_cache_meta: MockChainMetaBuf,
}

fn setup_env<'env>(
    reader: &'env Reader<'env>,
    dbs: &'env DbManager,
) -> DatabaseResult<Chains<'env>> {
    let previous_header = fake_header_hash("previous");

    let jimbo_id = fake_agent_pubkey("Jimbo");
    let jimbo_entry = Entry::Agent(jimbo_id.clone());
    let jessy_id = fake_agent_pubkey("Jessy");
    let jessy_entry = Entry::Agent(jessy_id.clone());

    let jimbo_header = ChainHeader::EntryCreate(header::EntryCreate {
        timestamp: chrono::Utc::now().timestamp().into(),
        author: jimbo_id.clone(),
        prev_header: previous_header.clone().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_address: jimbo_entry.entry_address(),
    });

    let jessy_header = ChainHeader::EntryCreate(header::EntryCreate {
        timestamp: chrono::Utc::now().timestamp().into(),
        author: jessy_id.clone(),
        prev_header: previous_header.clone().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_address: jessy_entry.entry_address(),
    });

    let source_chain = SourceChainBuf::new(reader, &dbs)?;
    let cache = SourceChainBuf::cache(reader, &dbs)?;
    let mock_primary_meta = MockChainMetaBuf::new();
    let mock_cache_meta = MockChainMetaBuf::new();
    Ok(Chains {
        source_chain,
        cache,
        jimbo_id,
        jimbo_header,
        jimbo_entry,
        jessy_id,
        jessy_header,
        jessy_entry,
        mock_primary_meta,
        mock_cache_meta,
    })
}

#[tokio::test]
async fn live_local_return() -> SourceChainResult<()> {
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id: _,
        jimbo_header,
        jimbo_entry,
        mut mock_primary_meta,
        mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    source_chain.put(jimbo_header.clone(), Some(jimbo_entry.clone()))?;
    let address = jimbo_entry.entry_address();

    // set it's metadata to LIVE
    mock_primary_meta
        .expect_get_crud()
        .with(predicate::eq(address.clone()))
        .returning(|_| Ok(EntryDhtStatus::Live));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(address.clone().into()).await?;
    // check it returns
    assert_eq!(entry.unwrap(), jimbo_entry);
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn dead_local_none() -> SourceChainResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id: _,
        jimbo_header,
        jimbo_entry,
        mut mock_primary_meta,
        mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    source_chain.put(jimbo_header.clone(), Some(jimbo_entry.clone()))?;
    let address = jimbo_entry.entry_address();

    // set it's metadata to Dead
    mock_primary_meta
        .expect_get_crud()
        .with(predicate::eq(address.clone()))
        .returning(|_| Ok(EntryDhtStatus::Dead));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(address.into()).await?;
    // check it returns none
    assert_eq!(entry, None);
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn notfound_goto_cache_live() -> SourceChainResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        source_chain,
        mut cache,
        jimbo_id: _,
        jimbo_header,
        jimbo_entry,
        mock_primary_meta,
        mut mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    cache.put(jimbo_header.clone(), Some(jimbo_entry.clone()))?;
    let address = jimbo_entry.entry_address();

    // set it's metadata to Live
    mock_cache_meta
        .expect_get_crud()
        .with(predicate::eq(address.clone()))
        .returning(|_| Ok(EntryDhtStatus::Live));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let _entry = cascade.dht_get(address).await?;
    // check it returns

    // FIXME!
    //    assert_eq!(entry, Some(jimbo));
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn notfound_cache() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        source_chain,
        cache,
        jimbo_header: _,
        jimbo_entry,
        mock_primary_meta,
        mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    let address = jimbo_entry.entry_address();

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
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    // check it doesn't ask the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn links_local_return() -> SourceChainResult<()> {
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id: _,
        jimbo_header,
        jimbo_entry,
        jessy_id: _,
        jessy_header,
        jessy_entry,
        mut mock_primary_meta,
        mock_cache_meta,
    } = setup_env(&reader, &dbs)?;
    source_chain.put(jimbo_header.clone(), Some(jimbo_entry.clone()))?;
    source_chain.put(jessy_header.clone(), Some(jessy_entry.clone()))?;
    let base = jimbo_entry.entry_address();
    let target = jessy_entry.entry_address();
    let result = target.clone();

    // Return a link between entries
    mock_primary_meta
        .expect_get_links()
        .with(
            predicate::eq(EntryAddress::from(base.clone())),
            predicate::eq("".to_string()),
        )
        .returning(move |_, _| Ok(hashset! {target.clone()}));

    // call dht_get_links with above base
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let links = cascade.dht_get_links(base.into(), "").await?;
    // check it returns
    assert_eq!(links, hashset! {result.into()});
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn links_cache_return() -> SourceChainResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id: _,
        jimbo_header,
        jimbo_entry,
        jessy_id: _,
        jessy_header,
        jessy_entry,
        mut mock_primary_meta,
        mut mock_cache_meta,
    } = setup_env(&reader, &dbs)?;
    source_chain.put(jimbo_header.clone(), Some(jimbo_entry.clone()))?;
    source_chain.put(jessy_header.clone(), Some(jessy_entry.clone()))?;
    let base = jimbo_entry.entry_address();
    let target = jessy_entry.entry_address();
    let result = target.clone();

    // Return empty links
    mock_primary_meta
        .expect_get_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| Ok(HashSet::new()));
    // Return a link between entries
    mock_cache_meta
        .expect_get_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| Ok(hashset! {target.clone().into()}));

    // call dht_get_links with above base
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let links = cascade.dht_get_links(base.into(), "").await?;
    // check it returns
    assert_eq!(links, hashset! {result.into()});
    Ok(())
}

#[tokio::test]
async fn links_notauth_cache() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        source_chain,
        cache,
        jimbo_header: _,
        jimbo_entry,
        jessy_id: _,
        jessy_header: _,
        jessy_entry,
        mock_primary_meta,
        mut mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;

    let base = jimbo_entry.entry_address();
    let target = jessy_entry.entry_address();
    let result = target.clone();

    // Return empty links
    mock_cache_meta
        .expect_get_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| Ok(hashset! {target.clone().into()}));

    // call dht_get_links with above base
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let links = cascade.dht_get_links(base.into(), "").await?;
    // check it returns
    assert_eq!(links, hashset! {result.into()});
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    Ok(())
}
