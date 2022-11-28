use holochain_state::source_chain::SourceChainRead;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{CapSecret, GrantZomeCallCapabilityPayload, RoleName};
use std::collections::BTreeSet;

use crate::fixt::AgentPubKeyFixturator;
use crate::sweettest::{SweetAgents, SweetConductor, SweetDnaFile};
use ::fixt::fixt;
use arbitrary::Arbitrary;

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn grant_zome_call_capability() {
    use holochain_zome_types::{CapAccess, ZomeCallCapGrant};

    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Create]).await;
    let role_name: RoleName = "dna".to_string();
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent(
            "app",
            agent_pub_key.clone(),
            [&(role_name.clone(), dna.clone())],
        )
        .await
        .unwrap();
    let cell_id = app.cells()[0].cell_id();

    // generate a cap access public key
    let cap_access_public_key = fixt!(AgentPubKey);

    // compute a cap access secret
    let mut buf = arbitrary::Unstructured::new(&[]);
    let cap_access_secret = CapSecret::arbitrary(&mut buf).unwrap();

    // set up functions to grant access to
    let mut functions = BTreeSet::new();
    let granted_function = ("zome".into(), "create".into());
    functions.insert(granted_function.clone());

    // set up assignees which is only the agent key
    let mut assignees = BTreeSet::new();
    assignees.insert(cap_access_public_key.clone());

    let cap_grant = ZomeCallCapGrant {
        tag: "signing_key".into(),
        functions,
        access: CapAccess::Assigned {
            secret: cap_access_secret,
            assignees,
        },
    };

    // request authorization of signing key for agent's own cell should succeed
    conductor
        .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id: cell_id.clone(),
            cap_grant: cap_grant.clone(),
        })
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

    let actual_cap_grant = source_chain_read
        .valid_cap_grant(
            granted_function.clone(),
            cap_access_public_key.clone(),
            Some(cap_access_secret.clone()),
        )
        .await
        .unwrap();

    assert!(actual_cap_grant.is_some());
    assert!(actual_cap_grant.unwrap().is_valid(
        &granted_function,
        &cap_access_public_key,
        Some(&cap_access_secret)
    ));
}
