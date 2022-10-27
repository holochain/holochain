use holochain_state::source_chain::SourceChainRead;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{AppRoleId, CapSecret, CapSecretBytes, CAP_SECRET_BYTES};
use rand::RngCore;
use std::collections::BTreeSet;

use crate::fixt::AgentPubKeyFixturator;
use crate::sweettest::{SweetAgents, SweetConductor, SweetDnaFile};
use ::fixt::fixt;

#[tokio::test(flavor = "multi_thread")]
async fn authorize_signing_key() {
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_id: AppRoleId = "dna".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent(
            "app",
            agent_pub_key.clone(),
            [&(role_id.clone(), dna.clone())],
        )
        .await
        .unwrap();
    let cell_id = app.cells()[0].cell_id();

    // generate a signing key
    let signing_key = fixt!(AgentPubKey);

    // compute a cap secret
    let mut buf: CapSecretBytes = [0; CAP_SECRET_BYTES];
    let mut rng = rand::thread_rng();
    rng.fill_bytes(&mut buf);
    let cap_secret = CapSecret(buf);

    // set up functions to grant access to
    let mut functions = BTreeSet::new();
    let granted_function = ("zome".into(), "create".into());
    functions.insert(granted_function.clone());

    conductor
        .authorize_zome_call_signing_key(
            holochain_types::prelude::AuthorizeZomeCallSigningKeyPayload {
                agent_pub_key: agent_pub_key.clone(),
                cell_id: cell_id.clone(),
                functions,
                signing_key: signing_key.clone(),
                cap_secret: cap_secret.clone(),
            },
        )
        .await
        .unwrap();

    // create a source chain read to query for the cap grant
    let authored_db = conductor.get_authored_db(cell_id.dna_hash()).unwrap();
    let dht_db = conductor.get_dht_db(cell_id.dna_hash()).unwrap();
    let dht_db_cache = conductor.get_dht_db_cache(cell_id.dna_hash()).unwrap();

    let source_chain_read = SourceChainRead::new(
        authored_db.into(),
        dht_db.into(),
        dht_db_cache,
        conductor.keystore(),
        agent_pub_key.clone(),
    )
    .await
    .unwrap();

    let signing_key_cap_grant = source_chain_read
        .valid_cap_grant(
            granted_function.clone(),
            signing_key.clone(),
            Some(cap_secret.clone()),
        )
        .await
        .unwrap();

    assert!(signing_key_cap_grant.is_some());
    assert!(signing_key_cap_grant.unwrap().is_valid(
        &granted_function,
        &signing_key,
        Some(&cap_secret)
    ));
}
