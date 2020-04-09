use super::Cascade;
use crate::core::state::{
    chain_meta::{EntryDhtStatus, MockChainMetaBuf},
    source_chain::SourceChainBuf,
};
use maplit::hashset;
use mockall::*;
use std::collections::HashSet;
use sx_state::{
    db::DbManager, env::ReadManager, error::DatabaseResult, prelude::Reader,
    test_utils::test_cell_env,
};
use sx_types::persistence::cas::content::Addressable;
use sx_types::{agent::AgentId, entry::Entry, observability};

struct Chains<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
    cache: SourceChainBuf<'env, Reader<'env>>,
    jimbo_id: AgentId,
    jimbo: Entry,
    jessy_id: AgentId,
    jessy: Entry,
    mock_primary_meta: MockChainMetaBuf,
    mock_cache_meta: MockChainMetaBuf,
}

fn setup_env<'env>(
    reader: &'env Reader<'env>,
    dbs: &'env DbManager,
) -> DatabaseResult<Chains<'env>> {
    let source_chain = SourceChainBuf::new(reader, &dbs)?;
    let cache = SourceChainBuf::cache(reader, &dbs)?;
    let jimbo_id = AgentId::generate_fake("jimbos_id");
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    let jessy_id = AgentId::generate_fake("jessy_id");
    let jessy = Entry::AgentId(AgentId::generate_fake("Jessy"));
    let mock_primary_meta = MockChainMetaBuf::new();
    let mock_cache_meta = MockChainMetaBuf::new();
    Ok(Chains {
        source_chain,
        cache,
        jimbo_id,
        jimbo,
        jessy_id,
        jessy,
        mock_primary_meta,
        mock_cache_meta,
    })
}

#[tokio::test]
async fn live_local_return() -> DatabaseResult<()> {
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id,
        jimbo,
        mut mock_primary_meta,
        mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    let address = jimbo.address();

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
    let entry = cascade.dht_get(&address).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn dead_local_none() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id,
        jimbo,
        mut mock_primary_meta,
        mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    let address = jimbo.address();

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
    let entry = cascade.dht_get(&address).await?;
    // check it returns none
    assert_eq!(entry, None);
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn notfound_goto_cache_live() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        source_chain,
        mut cache,
        jimbo_id,
        jimbo,
        mock_primary_meta,
        mut mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    cache.put_entry(jimbo.clone(), &jimbo_id);
    let address = jimbo.address();

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
    let entry = cascade.dht_get(&address).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
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
        jimbo,
        mock_primary_meta,
        mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    let address = jimbo.address();

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(&address).await?;
    // check it returns
    assert_eq!(entry, None);
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    // check it doesn't ask the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn links_local_return() -> DatabaseResult<()> {
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id,
        jimbo,
        jessy_id,
        jessy,
        mut mock_primary_meta,
        mock_cache_meta,
    } = setup_env(&reader, &dbs)?;
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    source_chain.put_entry(jessy.clone(), &jessy_id);
    let base = jimbo.address();
    let target = jessy.address();
    let result = target.clone();

    // Return a link between entries
    mock_primary_meta
        .expect_get_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| Ok(hashset! {target.clone()}));

    // call dht_get_links with above base
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let links = cascade.dht_get_links(base, "").await?;
    // check it returns
    assert_eq!(links, hashset! {result});
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn links_cache_return() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let Chains {
        mut source_chain,
        cache,
        jimbo_id,
        jimbo,
        jessy_id,
        jessy,
        mut mock_primary_meta,
        mut mock_cache_meta,
    } = setup_env(&reader, &dbs)?;
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    source_chain.put_entry(jessy.clone(), &jessy_id);
    let base = jimbo.address();
    let target = jessy.address();
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
        .returning(move |_, _| Ok(hashset! {target.clone()}));

    // call dht_get_links with above base
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let links = cascade.dht_get_links(base, "").await?;
    // check it returns
    assert_eq!(links, hashset! {result});
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
        jimbo,
        jessy,
        mock_primary_meta,
        mut mock_cache_meta,
        ..
    } = setup_env(&reader, &dbs)?;
    let base = jimbo.address();
    let target = jessy.address();
    let result = target.clone();

    // Return empty links
    mock_cache_meta
        .expect_get_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| Ok(hashset! {target.clone()}));

    // call dht_get_links with above base
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let links = cascade.dht_get_links(base, "").await?;
    // check it returns
    assert_eq!(links, hashset! {result});
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    Ok(())
}
