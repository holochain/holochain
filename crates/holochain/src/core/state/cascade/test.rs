use super::Cascade;
use crate::core::{
    net::MockNetRequester,
    state::{
        chain_meta::{Crud, MockChainMetaBuf},
        source_chain::SourceChainBuf,
    },
};
use mockall::*;
use sx_state::{env::ReadManager, error::DatabaseResult, test_utils::test_env};
use sx_types::{
    agent::AgentId, entry::Entry, observability, prelude::{Address, AddressableContent},
};
use std::collections::HashSet;
#[tokio::test]
async fn live_local_return() -> DatabaseResult<()> {
    // setup some data thats in the scratch
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;
    let jimbo_id = AgentId::generate_fake("jimbos_id");
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    let address = jimbo.address();

    // set it's metadata to LIVE
    let mut mock_primary_meta = MockChainMetaBuf::new();
    let mock_cache_meta = MockChainMetaBuf::new();
    mock_primary_meta
        .expect_get_crud()
        .with(predicate::eq(address.clone()))
        .returning(|_| Ok(Crud::Live));

    let mock_network = MockNetRequester::new();

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
        mock_network,
    );
    let entry = cascade.dht_get(address).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    // check it doesn't ask the network
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn dead_local_none() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;
    let jimbo_id = AgentId::generate_fake("jimbos_id");
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    let address = jimbo.address();

    // set it's metadata to Dead
    let mut mock_primary_meta = MockChainMetaBuf::new();
    let mock_cache_meta = MockChainMetaBuf::new();
    mock_primary_meta
        .expect_get_crud()
        .with(predicate::eq(address.clone()))
        .returning(|_| Ok(Crud::Dead));

    let mock_network = MockNetRequester::new();
    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
        mock_network,
    );
    let entry = cascade.dht_get(address).await?;
    // check it returns
    assert_eq!(entry, None);
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    // check it doesn't ask the network
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn notfound_goto_cache_live() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let mut cache = SourceChainBuf::cache(&reader, &dbs)?;
    let jimbo_id = AgentId::generate_fake("jimbos_id");
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    cache.put_entry(jimbo.clone(), &jimbo_id);
    let address = jimbo.address();

    // set it's metadata to Dead
    let mock_primary_meta = MockChainMetaBuf::new();
    let mut mock_cache_meta = MockChainMetaBuf::new();
    mock_cache_meta
        .expect_get_crud()
        .with(predicate::eq(address.clone()))
        .returning(|_| Ok(Crud::Live));

    let mock_network = MockNetRequester::new();
    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
        mock_network,
    );
    let entry = cascade.dht_get(address).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
    // check it doesn't ask the network
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn notfound_cache_notfound_network() -> DatabaseResult<()> {
    observability::test_run().ok();
    // setup some data thats in the scratch
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    let address = jimbo.address();
    let jimbo_net = jimbo.clone();

    // set it's metadata to Dead
    let mock_primary_meta = MockChainMetaBuf::new();
    let mock_cache_meta = MockChainMetaBuf::new();
    let mut mock_network = MockNetRequester::new();

    mock_network
        .expect_fetch_entry()
        .with(predicate::eq(address.clone()))
        .returning(move |_| Ok(Some(jimbo_net.clone())));

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
        mock_network,
    );
    let entry = cascade.dht_get(address).await?;
    // check it returns
    assert_eq!(entry, Some(jimbo));
    // check it doesn't ask the network
    // this is implied by the mock not expecting calls
    Ok(())
}

#[tokio::test]
async fn links_local_return() -> DatabaseResult<()> {
    // setup some data thats in the scratch
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;
    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;
    let jimbo_id = AgentId::generate_fake("jimbos_id");
    let jimbo = Entry::AgentId(AgentId::generate_fake("Jimbo"));
    let jessy_id = AgentId::generate_fake("jessy_id");
    let jessy = Entry::AgentId(AgentId::generate_fake("Jessy"));
    source_chain.put_entry(jimbo.clone(), &jimbo_id);
    source_chain.put_entry(jessy.clone(), &jessy_id);
    let base = jimbo.address();
    let target = jessy.address();
    let result = target.clone();

    // Return a link between entries
    let mut mock_primary_meta = MockChainMetaBuf::new();
    let mock_cache_meta = MockChainMetaBuf::new();
    mock_primary_meta
        .expect_get_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| {
            Ok([target.clone()]
                .iter()
                .cloned()
                .collect::<HashSet<Address>>())
        });

    let mock_network = MockNetRequester::new();

    // call dht_get with above address
    let cascade = Cascade::new(
        &source_chain.cas(),
        &mock_primary_meta,
        &cache.cas(),
        &mock_cache_meta,
        mock_network,
    );
    let links = cascade.dht_get_links(base, "").await?;
    let link = links.into_iter().next();
    // check it returns
    assert_eq!(link, Some(result));
    // check it doesn't hit the cache
    // this is implied by the mock not expecting calls
    // check it doesn't ask the network
    // this is implied by the mock not expecting calls
    Ok(())
}
