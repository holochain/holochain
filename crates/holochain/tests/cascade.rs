use holochain_2020::core::{
    net::MockNetRequester,
    state::{cascade::Cascade, chain_meta::ChainMetaBuf, source_chain::SourceChainBuf},
};
use mockall::*;
use std::collections::HashSet;
use sx_state::{env::ReadManager, error::DatabaseResult, test_utils::test_env};
use sx_types::{agent::AgentId, entry::Entry, persistence::cas::content::AddressableContent, prelude::Address};

#[tokio::test]
async fn get_links() -> DatabaseResult<()> {
    let env = test_env();
    let dbs = env.dbs().await?;
    let env_ref = env.guard().await;
    let reader = env_ref.reader()?;

    let mut source_chain = SourceChainBuf::new(&reader, &dbs)?;
    let cache = SourceChainBuf::cache(&reader, &dbs)?;

    // create a cache and a cas for store and meta
    let primary_meta = ChainMetaBuf::primary(&reader, &dbs)?;
    let cache_meta = ChainMetaBuf::cache(&reader, &dbs)?;

    let jimbo_id = AgentId::generate_fake("Jimbo");
    let jimbo = Entry::AgentId(jimbo_id.clone());
    let jessy_id = AgentId::generate_fake("jessy_id");
    let jessy = Entry::AgentId(AgentId::generate_fake("Jessy"));
    let base = jimbo.address();
    let target = jessy.address();
    let result = target.clone();
    source_chain.put_entry(jimbo, &jimbo_id);
    source_chain.put_entry(jessy, &jessy_id);

    let mut mock_network = MockNetRequester::new();
    mock_network
        .expect_fetch_links()
        .with(predicate::eq(base.clone()), predicate::eq("".to_string()))
        .returning(move |_, _| {
            Ok([target.clone()]
                .iter()
                .cloned()
                .collect::<HashSet<Address>>())
        });

    // Pass in stores as references
    let cascade = Cascade::new(
        &source_chain.cas(),
        &primary_meta,
        &cache.cas(),
        &cache_meta,
        mock_network,
    );
    let links = cascade.dht_get_links(base, "").await?;
    let link = links.into_iter().next();
    assert_eq!(link, Some(result));
    Ok(())
}
