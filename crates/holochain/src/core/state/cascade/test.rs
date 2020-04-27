use super::Cascade;
use crate::core::state::{
    chain_meta::{EntryDhtStatus, MockChainMetaBuf},
    source_chain::SourceChainBuf,
};
use holo_hash::EntryHash;
use holochain_state::{
    db::DbManager, env::ReadManager, error::DatabaseResult, prelude::Reader,
    test_utils::test_cell_env,
};
use holochain_types::entry::EntryAddress;
use holochain_types::{entry::Entry, observability, prelude::*};
use maplit::hashset;
use mockall::*;
use std::collections::HashSet;
use std::convert::TryInto;

struct Chains<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
    cache: SourceChainBuf<'env, Reader<'env>>,
    jimbo_id: AgentHash,
    jimbo: Entry,
    jessy_id: AgentHash,
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
    let jimbo_id: AgentHash = "uhCAkw-zrttiYpdfAYX4fR6W8DPUdheZJ-1QsRA4cTImmzTYUcOr4"
        .try_into()
        .unwrap();
    let jimbo = Entry::AgentKey(jimbo_id.clone());
    let jessy_id: AgentHash = "uhCAkomHzekU0-x7p62WmrusdxD2w9wcjdajC88688JGSTEo6cbEK"
        .try_into()
        .unwrap();
    let jessy = Entry::AgentKey(jessy_id.clone());
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

/*
#[tokio::test(threaded_scheduler)]
async fn live_local_return() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    source_chain.put_entry(jimbo.clone(), &jimbo_id).await?;
    let hash: EntryHash = (&jimbo).try_into()?;

    // set it's metadata to LIVE
    mock_primary_meta
        .expect_get_crud()
        .with(predicate::eq(EntryAddress::from(hash.clone())))
        .returning(|_| Ok(EntryDhtStatus::Live));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(hash.clone().into()).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
    }).await.unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn dead_local_none() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    source_chain.put_entry(jimbo.clone(), &jimbo_id).await?;
    let hash: EntryHash = jimbo.try_into()?;

    // set it's metadata to Dead
    mock_primary_meta
        .expect_get_crud()
        .with(predicate::eq(EntryAddress::from(hash.clone())))
        .returning(|_| Ok(EntryDhtStatus::Dead));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(hash.into()).await?;
    // check it returns none
    assert_eq!(entry, None);
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    Ok(())
    }).await.unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn notfound_goto_cache_live() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    cache.put_entry(jimbo.clone(), &jimbo_id).await?;
    let hash: EntryHash = (&jimbo).try_into()?;

    // set it's metadata to Live
    mock_cache_meta
        .expect_get_crud()
        .with(predicate::eq(EntryAddress::from(hash.clone())))
        .returning(|_| Ok(EntryDhtStatus::Live));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(hash.into()).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    Ok(())
    }).await.unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn notfound_cache() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    let hash: EntryHash = jimbo.try_into()?;

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
    );
    let entry = cascade.dht_get(hash.into()).await?;
    // check it returns
    assert_eq!(entry, None);
    // check it doesn't hit the primary
    // this is implied by the mock not expecting calls
    // check it doesn't ask the cache
    // this is implied by the mock not expecting calls
    Ok(())
    }).await.unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn links_local_return() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    source_chain.put_entry(jimbo.clone(), &jimbo_id).await?;
    source_chain.put_entry(jessy.clone(), &jessy_id).await?;
    let base: EntryHash = jimbo.try_into()?;
    let target: EntryHash = jessy.try_into()?;
    let result = target.clone();

    // Return a link between entries
    mock_primary_meta
        .expect_get_links()
        .with(
            predicate::eq(EntryAddress::from(base.clone())),
            predicate::eq("".to_string()),
        )
        .returning(move |_, _| Ok(hashset! {EntryAddress::from(target.clone())}));

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
    }).await.unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn links_cache_return() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    source_chain.put_entry(jimbo.clone(), &jimbo_id).await?;
    source_chain.put_entry(jessy.clone(), &jessy_id).await?;
    let base: EntryHash = jimbo.try_into()?;
    let target: EntryHash = jessy.try_into()?;
    let result = target.clone();

    // Return empty links
    mock_primary_meta
        .expect_get_links()
        .with(
            predicate::eq(EntryAddress::from(base.clone())),
            predicate::eq("".to_string()),
        )
        .returning(move |_, _| Ok(HashSet::new()));
    // Return a link between entries
    mock_cache_meta
        .expect_get_links()
        .with(
            predicate::eq(EntryAddress::from(base.clone())),
            predicate::eq("".to_string()),
        )
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
    }).await.unwrap()
}

#[tokio::test(threaded_scheduler)]
async fn links_notauth_cache() -> DatabaseResult<()> {
    tokio::task::spawn(async move {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_cell_env().await;
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
    let base: EntryHash = jimbo.try_into()?;
    let target: EntryHash = jessy.try_into()?;
    let result = target.clone();

    // Return empty links
    mock_cache_meta
        .expect_get_links()
        .with(
            predicate::eq(EntryAddress::from(base.clone())),
            predicate::eq("".to_string()),
        )
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
    }).await.unwrap()
}
*/
