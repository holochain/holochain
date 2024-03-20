use holochain_nonce::Nonce256Bits;
use holochain_state::source_chain::SourceChainRead;
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::prelude::*;
use std::collections::BTreeSet;

use crate::fixt::AgentPubKeyFixturator;
use crate::sweettest::{SweetAgents, SweetConductor, SweetDnaFile};
use ::fixt::fixt;
use arbitrary::Arbitrary;

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn signed_zome_call() {
    use holochain_conductor_api::ZomeCall;
    use holochain_nonce::fresh_nonce;
    use matches::assert_matches;

    let zome = TestWasm::Create;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![zome]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", agent_pub_key.clone(), [&dna])
        .await
        .unwrap();
    let cell_id = app.cells()[0].cell_id();

    // generate a cap access public key
    let cap_access_public_key = fixt!(AgentPubKey, ::fixt::Predictable, 1);

    // compute a cap access secret
    let mut buf = arbitrary::Unstructured::new(&[]);
    let cap_access_secret = CapSecret::arbitrary(&mut buf).unwrap();

    // set up functions to grant access to
    let mut functions = BTreeSet::new();
    let granted_function: GrantedFunction = ("create_entry".into(), "get_entry".into());
    functions.insert(granted_function.clone());
    let granted_functions = GrantedFunctions::Listed(functions);
    // set up assignees which is only the agent key
    let mut assignees = BTreeSet::new();
    assignees.insert(cap_access_public_key.clone());

    let cap_grant = ZomeCallCapGrant {
        tag: "signing_key".into(),
        functions: granted_functions,
        access: CapAccess::Assigned {
            secret: cap_access_secret,
            assignees,
        },
    };

    // request authorization of signing key for agent's own cell should succeed
    let grant_action_hash = conductor
        .grant_zome_call_capability(GrantZomeCallCapabilityPayload {
            cell_id: cell_id.clone(),
            cap_grant: cap_grant.clone(),
        })
        .await
        .unwrap();

    // create a source chain read to query for the cap grant
    let authored_db = conductor
        .get_or_create_authored_db(cell_id.dna_hash(), cell_id.agent_pubkey().clone())
        .unwrap();
    let dht_db = conductor.get_dht_db(cell_id.dna_hash()).unwrap();
    let dht_db_cache = conductor.get_dht_db_cache(cell_id.dna_hash()).unwrap();

    let chain = SourceChainRead::new(
        authored_db.into(),
        dht_db.into(),
        dht_db_cache,
        conductor.keystore(),
        agent_pub_key.clone(),
    )
    .await
    .unwrap();

    let head = chain.chain_head_nonempty().unwrap();
    let dump = chain.dump().await.unwrap();

    dump.records.into_iter().for_each(|r| {
        let seq = r.action.action_seq();
        let hash = r.action_address;
        let ty = r.action.action_type();
        if let Some(e) = r.entry {
            println!("{seq:3} {ty:16 } {hash} {e:?}");
        } else {
            println!("{seq:3} {ty:16 } {hash}");
        }
    });

    // Genesis entries are 0, 1, and 2.
    // 3 is the cap grant created during init in the test wasm.
    // 4 is InitZomesComplete.
    // 5 is this grant added via admin call.
    // This checks that init ran before the grant was created.
    assert_eq!(head.seq, 5);
    assert_eq!(head.action, grant_action_hash);

    let actual_cap_grant = chain
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

    // a zome call without the cap secret that enables lookup of the authorized
    // signing key should be rejected
    let response = conductor
        .call_zome(
            ZomeCall::try_from_unsigned_zome_call(
                &conductor.keystore(),
                ZomeCallUnsigned {
                    provenance: cap_access_public_key.clone(), // N.B.: using agent key would bypass capgrant lookup
                    cell_id: cell_id.clone(),
                    zome_name: zome.coordinator_zome_name(),
                    fn_name: "get_entry".into(),
                    cap_secret: None,
                    payload: ExternIO::encode(()).unwrap(),
                    nonce: Nonce256Bits::from([0; 32]),
                    expires_at: Timestamp(Timestamp::now().as_micros() + 100000),
                },
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_matches!(response, ZomeCallResponse::Unauthorized(..));

    // a zome call with the cap secret of the authorized signing key should succeed
    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    let response = conductor
        .call_zome(
            ZomeCall::try_from_unsigned_zome_call(
                &conductor.keystore(),
                ZomeCallUnsigned {
                    provenance: cap_access_public_key.clone(), // N.B.: using agent key would bypass capgrant lookup
                    cell_id: cell_id.clone(),
                    zome_name: zome.coordinator_zome_name(),
                    fn_name: "get_entry".into(),
                    cap_secret: Some(cap_access_secret),
                    payload: ExternIO::encode(()).unwrap(),
                    nonce,
                    expires_at,
                },
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_matches!(response, ZomeCallResponse::Ok(_));
}

#[tokio::test(flavor = "multi_thread")]
#[cfg(feature = "test_utils")]
async fn signed_zome_call_wildcard() {
    use holochain_conductor_api::ZomeCall;
    use holochain_nonce::fresh_nonce;
    use holochain_zome_types::prelude::*;
    use matches::assert_matches;

    let zome = TestWasm::Create;
    let (dna, _, _) = SweetDnaFile::unique_from_test_wasms(vec![zome]).await;
    let mut conductor = SweetConductor::from_standard_config().await;
    let agent_pub_key = SweetAgents::one(conductor.keystore()).await;
    let app = conductor
        .setup_app_for_agent("app", agent_pub_key.clone(), [&dna])
        .await
        .unwrap();
    let cell_id = app.cells()[0].cell_id();

    // generate a cap access public key
    let cap_access_public_key = fixt!(AgentPubKey, ::fixt::Predictable, 1);

    // compute a cap access secret
    let mut buf = arbitrary::Unstructured::new(&[]);
    let cap_access_secret = CapSecret::arbitrary(&mut buf).unwrap();

    // set up functions to grant access to
    let granted_functions = GrantedFunctions::All;

    // set up assignees which is only the agent key
    let mut assignees = BTreeSet::new();
    assignees.insert(cap_access_public_key.clone());

    let cap_grant = ZomeCallCapGrant {
        tag: "signing_key".into(),
        functions: granted_functions,
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
    let authored_db = conductor
        .get_or_create_authored_db(cell_id.dna_hash(), cell_id.agent_pubkey().clone())
        .unwrap();
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

    let called_function: GrantedFunction = ("create_entry".into(), "get_entry".into());

    let actual_cap_grant = source_chain_read
        .valid_cap_grant(
            called_function.clone(),
            cap_access_public_key.clone(),
            Some(cap_access_secret.clone()),
        )
        .await
        .unwrap();
    assert!(actual_cap_grant.is_some());
    assert!(actual_cap_grant.unwrap().is_valid(
        &called_function,
        &cap_access_public_key,
        Some(&cap_access_secret)
    ));

    // a zome call with the cap secret of the authorized signing key should succeed
    let (nonce, expires_at) = fresh_nonce(Timestamp::now()).unwrap();
    let response = conductor
        .call_zome(
            ZomeCall::try_from_unsigned_zome_call(
                &conductor.keystore(),
                ZomeCallUnsigned {
                    provenance: cap_access_public_key.clone(), // N.B.: using agent key would bypass capgrant lookup
                    cell_id: cell_id.clone(),
                    zome_name: zome.coordinator_zome_name(),
                    fn_name: "get_entry".into(),
                    cap_secret: Some(cap_access_secret),
                    payload: ExternIO::encode(()).unwrap(),
                    nonce,
                    expires_at,
                },
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_matches!(response, ZomeCallResponse::Ok(_));
}
