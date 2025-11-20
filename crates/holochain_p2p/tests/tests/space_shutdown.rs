use crate::tests::common::Handler;
use holo_hash::{AgentPubKey, DnaHash};
use holochain_keystore::test_keystore;
use holochain_p2p::{spawn_holochain_p2p, HolochainP2pConfig};
use holochain_state::prelude::{test_conductor_db, test_dht_db, test_peer_meta_store_db};
use kitsune2_api::LocalAgent;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn space_shutdown() {
    let dna_hash = DnaHash::from_raw_36(vec![0; 36]);
    let space_id = dna_hash.to_k2_space();

    let dht_db = test_dht_db().to_db();
    let conductor_db = test_conductor_db().to_db();
    let peer_meta_db = test_peer_meta_store_db(dna_hash.clone()).to_db();

    let keystore = test_keystore();

    let p2p = spawn_holochain_p2p(
        HolochainP2pConfig {
            network_config: Some(serde_json::json!({
                "coreBootstrap": {
                    "serverUrl": "https://not_a_host"
                },
                "tx5Transport": {
                    "serverUrl": "wss://not_a_host"
                }
            })),
            get_db_op_store: Arc::new(move |_space| {
                let dht_db = dht_db.clone();
                Box::pin(async move { Ok(dht_db) })
            }),
            get_conductor_db: Arc::new(move || {
                let conductor_db = conductor_db.clone();
                Box::pin(async move { conductor_db })
            }),
            get_db_peer_meta: Arc::new(move |_space| {
                let peer_meta_db = peer_meta_db.clone();
                Box::pin(async move { Ok(peer_meta_db) })
            }),
            ..HolochainP2pConfig::default()
        },
        keystore.clone(),
    )
    .await
    .unwrap();

    p2p.register_handler(Arc::new(Handler::default()))
        .await
        .unwrap();

    let agent = keystore.new_sign_keypair_random().await.unwrap();
    p2p.join(dna_hash.clone(), agent.clone(), None)
        .await
        .unwrap();

    // Verify the setup, with a space that exists and one local agent
    let space = p2p.test_kitsune().space_if_exists(space_id.clone()).await;
    assert!(space.is_some(), "Space should exist after initial setup");
    let local_agents = space.unwrap().local_agent_store().get_all().await.unwrap();
    assert_eq!(1, local_agents.len(), "There should be one local agent");
    let local_agent_pub_key = AgentPubKey::from_k2_agent(local_agents[0].agent());
    assert_eq!(
        agent, local_agent_pub_key,
        "Local agent pub key should match"
    );

    // Now leave the space, which should trigger a shutdown of the space
    p2p.leave(dna_hash.clone(), agent.clone()).await.unwrap();

    // Verify the space is gone
    let space = p2p.test_kitsune().space_if_exists(space_id.clone()).await;
    assert!(
        space.is_none(),
        "Space should be gone after last agent leaves"
    );
}
