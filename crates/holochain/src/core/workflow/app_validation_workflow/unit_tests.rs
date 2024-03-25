use crate::{
    conductor::space::TestSpace,
    core::{
        ribosome::{
            guest_callback::validate::ValidateInvocation, real_ribosome::RealRibosome,
            ZomesToInvoke,
        },
        workflow::app_validation_workflow::{
            put_integrated, run_validation_callback_inner, Outcome,
        },
    },
    sweettest::{SweetDnaFile, SweetInlineZomes},
    test_utils::{test_network, test_network_with_events},
};
use fixt::fixt;
use holo_hash::{hash_type::AnyDht, AnyDhtHash, DhtOpHash, HashableContentExtSync};
use holochain_keystore::test_keystore;
use holochain_p2p::{actor::HolochainP2pRefToDna, event::HolochainP2pEvent, stub_network};
use holochain_state::{host_fn_workspace::HostFnWorkspaceRead, mutations::insert_op};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed, WireOps},
    record::{SignedActionHashedExt, WireRecordOps},
};
use holochain_wasmer_host::module::ModuleCache;
use holochain_zome_types::{
    action::{ActionHashed, Dna},
    chain::{ChainFilter, MustGetAgentActivityInput},
    dependencies::holochain_integrity_types::{UnresolvedDependencies, ValidateCallbackResult},
    entry::MustGetActionInput,
    fixt::{ActionFixturator, CreateFixturator, DnaHashFixturator, SignatureFixturator},
    judged::Judged,
    op::{Op, RegisterAgentActivity},
    record::SignedActionHashed,
    timestamp::Timestamp,
    validate::ValidationStatus,
    Action,
};
use kitsune_p2p_types::ok_fut;
use matches::assert_matches;
use parking_lot::RwLock;
use std::{sync::Arc, time::Duration};

// test app validation with a must get action
// where initially the action is not in the cache db
// and is then added to it
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_must_get_action() {
    let zomes =
        SweetInlineZomes::new(vec![], 0).integrity_function("validate", move |api, op: Op| {
            if let Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) = op {
                if let Ok(action) = api.must_get_action(MustGetActionInput::new(action.to_hash())) {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![action.to_hash().into()]),
                    ))
                }
            } else {
                Ok(ValidateCallbackResult::Invalid("wrong op type".to_string()))
            }
        });

    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zomes_to_invoke = ZomesToInvoke::one_integrity(integrity_zomes[0].clone());

    let keystore = test_keystore();
    let agent_key = keystore.new_sign_keypair_random().await.unwrap();
    let test_space = TestSpace::new(dna_file.dna_hash().to_owned());

    let mut create = fixt!(Create);
    create.author = agent_key.clone();
    let create_action = Action::Create(create.clone());
    let action_hashed = ActionHashed::from_content_sync(Action::Create(create));
    let signed_action = SignedActionHashed::sign(&keystore, action_hashed)
        .await
        .unwrap();
    let op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: signed_action,
        cached_entry: None,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &op).unwrap();

    let ribosome = RealRibosome::new(
        dna_file.clone(),
        Arc::new(RwLock::new(ModuleCache::new(None))),
    )
    .unwrap();

    let workspace_read = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(agent_key.clone())
            .unwrap()
            .into(),
        test_space.space.dht_db.into(),
        test_space.space.dht_query_cache,
        test_space.space.cache_db.clone().into(),
        keystore.clone(),
        None,
        Arc::new(dna_file.dna_def().to_owned()),
    )
    .await
    .unwrap();

    let network = stub_network()
        .await
        .to_dna(dna_file.dna_hash().to_owned(), None);

    // action has not been written to a database yet
    // validation should indicate it is awaiting create action hash
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![create_action.to_hash().into()]);

    // write action to be must got during validation to dht cache db
    let dht_op = DhtOp::RegisterAgentActivity(fixt!(Signature), create_action.clone());
    let dht_op_hashed = DhtOpHashed::from_content_sync(dht_op);
    test_space.space.cache_db.test_write(move |txn| {
        insert_op(txn, &dht_op_hashed).unwrap();
    });

    // the same validation should now be successfully validating the op
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}

// test that unresolved dependency hashes are fetched
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_awaiting_deps_hashes() {
    holochain_trace::test_run().unwrap();

    let keystore = test_keystore();
    let agent_key = keystore.new_sign_keypair_random().await.unwrap();

    let action = fixt!(Action);

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        let action_hash = action.clone().to_hash();
        move |api, op: Op| {
            if let Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) = op {
                let result = api.must_get_action(MustGetActionInput(action.as_hash().to_owned()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![action_hash.clone().into()]),
                    ))
                }
            } else {
                Ok(ValidateCallbackResult::Valid)
            }
        }
    });

    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zomes_to_invoke = ZomesToInvoke::OneIntegrity(integrity_zomes[0].clone());
    let dna_hash = dna_file.dna_hash().clone();

    let test_space = TestSpace::new(dna_hash.clone());

    let action_signed_hashed = SignedActionHashed::new_unchecked(action.clone(), fixt!(Signature));
    let action_op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: action_signed_hashed.clone(),
        cached_entry: None,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &action_op).unwrap();

    let ribosome = RealRibosome::new(
        dna_file.clone(),
        Arc::new(RwLock::new(ModuleCache::new(None))),
    )
    .unwrap();

    let workspace_read = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(agent_key.clone())
            .unwrap()
            .into(),
        test_space.space.dht_db.into(),
        test_space.space.dht_query_cache,
        test_space.space.cache_db.into(),
        keystore.clone(),
        None,
        Arc::new(dna_file.dna_def().to_owned()),
    )
    .await
    .unwrap();

    // handle only Get events
    let filter_events = |evt: &_| match evt {
        holochain_p2p::event::HolochainP2pEvent::Get { .. } => true,
        _ => false,
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let network = test_network_with_events(
        Some(dna_hash.clone()),
        Some(agent_key.clone()),
        filter_events,
        tx,
    )
    .await;

    // respond to Get request with requested action
    let action_hash = action.clone().to_hash();
    let action_hash_32 = action_hash.get_raw_32().to_vec();
    tokio::spawn({
        async move {
            while let Some(evt) = rx.recv().await {
                if let HolochainP2pEvent::Get {
                    dht_hash, respond, ..
                } = evt
                {
                    assert_eq!(dht_hash.get_raw_32().to_vec(), action_hash_32);

                    respond.r(ok_fut(Ok(WireOps::Record(WireRecordOps {
                        action: Some(Judged::new(
                            action_signed_hashed.clone().into(),
                            ValidationStatus::Valid,
                        )),
                        deletes: vec![],
                        updates: vec![],
                        entry: None,
                    }))))
                }
            }
        }
    });

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
    )
    .await
    .unwrap();
    let random_action_hash = action.clone().to_hash();
    let random_action_hash_32 = random_action_hash.get_raw_32().to_vec();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![AnyDhtHash::from_raw_32_and_type(random_action_hash_32, AnyDht::Action)]);

    // await while missing record is being fetched in background task
    tokio::time::sleep(Duration::from_millis(500)).await;

    // app validation outcome should be accepted, now that the missing record
    // has been fetched
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted)
}

// test that unresolved dependency hashes are fetched
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_awaiting_deps_agent_activity() {
    holochain_trace::test_run().unwrap();

    let keystore = test_keystore();
    let alice = keystore.new_sign_keypair_random().await.unwrap();
    let bob = keystore.new_sign_keypair_random().await.unwrap();

    let action = Action::Dna(Dna {
        author: bob.clone(),
        timestamp: Timestamp::now(),
        hash: fixt!(DnaHash),
    });

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) = op {
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: action.hashed.author().to_owned(),
                    chain_filter: ChainFilter::new(action.as_hash().to_owned()),
                });
                println!("must get agent activity result is {result:?}");
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(
                            action.hashed.author().to_owned(),
                            ChainFilter::new(action.as_hash().to_owned()),
                        ),
                    ))
                }
            } else {
                Ok(ValidateCallbackResult::Valid)
            }
        }
    });

    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zomes_to_invoke = ZomesToInvoke::OneIntegrity(integrity_zomes[0].clone());
    let dna_hash = dna_file.dna_hash().clone();

    let test_space = TestSpace::new(dna_hash.clone());

    let action_signed_hashed = SignedActionHashed::new_unchecked(action.clone(), fixt!(Signature));
    let action_op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: action_signed_hashed.clone(),
        cached_entry: None,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &action_op).unwrap();

    let ribosome = RealRibosome::new(
        dna_file.clone(),
        Arc::new(RwLock::new(ModuleCache::new(None))),
    )
    .unwrap();

    let workspace_read = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(alice.clone())
            .unwrap()
            .into(),
        test_space.space.dht_db.into(),
        test_space.space.dht_query_cache,
        test_space.space.cache_db.clone().into(),
        keystore.clone(),
        None,
        Arc::new(dna_file.dna_def().to_owned()),
    )
    .await
    .unwrap();

    // handle only MustGetAgentActivity events
    // let filter_events = |evt: &_| match evt {
    //     holochain_p2p::event::HolochainP2pEvent::MustGetAgentActivity { .. } => true,
    //     _ => false,
    // };
    // let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    // let network = test_network_with_events(
    //     Some(dna_hash.clone()),
    //     Some(alice.clone()),
    //     filter_events,
    //     tx,
    // )
    // .await;

    // respond to MustGetAgentActivity request with bob's activity
    // tokio::spawn({
    //     let bob = bob.clone();
    //     async move {
    //         println!("spawning this");
    //         while let Some(evt) = rx.recv().await {
    //             println!("event is {evt:?}");
    //             if let HolochainP2pEvent::MustGetAgentActivity {
    //                 respond, author, ..
    //             } = evt
    //             {
    //                 assert_eq!(author, bob);

    //                 respond.r(ok_fut(Ok(MustGetAgentActivityResponse::Activity(vec![
    //                     RegisterAgentActivity {
    //                         action: action_signed_hashed.clone(),
    //                         cached_entry: None,
    //                     },
    //                 ]))))
    //             }
    //         }
    //     }
    // });

    let network = test_network(Some(dna_hash.clone()), Some(alice.clone())).await;

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![action.author().to_owned().into()]);

    // alice is an authority for bob, so must_get_agent_activity will not go to
    // the network and return IncompleteChain instead
    // therefore write the op manually to alice's cache db
    let dht_op = DhtOp::RegisterAgentActivity(action_signed_hashed.signature, action.clone());
    let dht_op_hash = DhtOpHash::with_data_sync(&dht_op);
    let dht_op_hashed = DhtOpHashed::with_pre_hashed(dht_op, dht_op_hash.clone());
    test_space.space.cache_db.test_write(move |txn| {
        insert_op(txn, &dht_op_hashed).unwrap();
        put_integrated(txn, &dht_op_hash, ValidationStatus::Valid).unwrap();
    });

    // app validation outcome should be accepted, now that the missing agent
    // activity is available
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}
