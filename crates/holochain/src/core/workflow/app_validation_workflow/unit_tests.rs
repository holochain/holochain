use crate::{
    conductor::space::TestSpace,
    core::{
        ribosome::{
            guest_callback::validate::ValidateInvocation, real_ribosome::RealRibosome,
            ZomesToInvoke,
        },
        workflow::app_validation_workflow::{
            put_integrated, run_validation_callback, Outcome, ValidationDependencies,
        },
    },
    fixt::MetaLairClientFixturator,
    sweettest::{SweetDnaFile, SweetInlineZomes},
    test_utils::{test_network, test_network_with_events},
};
use fixt::fixt;
use holo_hash::{hash_type::AnyDht, AgentPubKey, AnyDhtHash, DhtOpHash, HashableContentExtSync};
use holochain_keystore::{test_keystore, MetaLairClient};
use holochain_p2p::{event::HolochainP2pEvent, HolochainP2pDnaFixturator};
use holochain_state::{host_fn_workspace::HostFnWorkspaceRead, mutations::insert_op};
use holochain_types::{
    action::WireDelete,
    dht_op::{DhtOp, DhtOpHashed, WireOps},
    dna::DnaFile,
    record::{SignedActionHashedExt, WireRecordOps},
};
use holochain_wasmer_host::module::ModuleCache;
use holochain_zome_types::{
    action::{ActionHashed, ActionType, Create, Dna},
    chain::{ChainFilter, MustGetAgentActivityInput},
    dependencies::holochain_integrity_types::{UnresolvedDependencies, ValidateCallbackResult},
    dna_def::DnaDef,
    entry::MustGetActionInput,
    fixt::{
        ActionFixturator, AgentPubKeyFixturator, CreateFixturator, DeleteFixturator,
        DnaHashFixturator, SignatureFixturator,
    },
    judged::Judged,
    op::{EntryCreationAction, Op, RegisterAgentActivity, RegisterDelete},
    record::{SignedActionHashed, SignedHashed},
    timestamp::Timestamp,
    validate::ValidationStatus,
    zome::{IntegrityZomeDef, Zome},
    Action,
};
use kitsune_p2p_types::ok_fut;
use matches::assert_matches;
use parking_lot::{Mutex, RwLock};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicI8, Ordering},
        Arc,
    },
    time::Duration,
};

// test app validation with a must get action where the original action of
// a delete is not in the cache db and then added to it
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_must_get_action() {
    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete {
                original_action, ..
            }) = op
            {
                let result = api.must_get_action(MustGetActionInput(original_action.to_hash()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![original_action.to_hash().into()]),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        dna_file,
        integrity_zomes,
        ribosome,
        test_space,
    } = TestCase::new(zomes).await;

    let alice = fixt!(AgentPubKey);
    let bob = fixt!(AgentPubKey);

    // a create
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = Action::Create(create.clone());
    let action_signed_hashed = SignedHashed::new_unchecked(create_action.clone(), fixt!(Signature));
    let action_op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: action_signed_hashed.clone(),
        cached_entry: None,
    });

    // a delete that references the create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    let delete_action = Action::Delete(delete.clone());
    let delete_signed_hashed = SignedHashed::new_unchecked(delete.clone(), fixt!(Signature));
    let delete_op = Op::RegisterDelete(RegisterDelete {
        delete: delete_signed_hashed.clone(),
        original_action: EntryCreationAction::Create(create.clone()),
        original_entry: None,
    });

    let network = fixt!(HolochainP2pDna);
    let workspace_read = get_workspace_read(&test_space, &alice, dna_file.dna_def()).await;
    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // action has not been written to a database yet
    // validation should indicate it is awaiting create action hash
    let outcome = run_validation_callback(
        &delete_op,
        &ribosome,
        workspace_read.clone(),
        network.clone(),
        fetched_dependencies.clone(),
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

    // the same validation should now successfully validate the op
    let outcome = run_validation_callback(
        &delete_op,
        &ribosome,
        workspace_read.clone(),
        network.clone(),
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}

// same as previous test but this time awaiting the background task that
// fetches the missing original create of a delete
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_awaiting_deps_hashes() {
    holochain_trace::test_run().unwrap();

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete {
                original_action, ..
            }) = op
            {
                let result = api.must_get_action(MustGetActionInput(original_action.to_hash()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![original_action.to_hash().into()]),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        dna_file,
        integrity_zomes,
        ribosome,
        test_space,
    } = TestCase::new(zomes).await;

    let alice = fixt!(AgentPubKey);
    let bob = fixt!(AgentPubKey);

    // a create
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = Action::Create(create.clone());
    let create_action_signed_hashed =
        SignedHashed::new_unchecked(create_action.clone(), fixt!(Signature));
    let create_action_op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: create_action_signed_hashed.clone(),
        cached_entry: None,
    });

    // a delete that references the create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    let delete_action = Action::Delete(delete.clone());
    let delete_signed_hashed = SignedHashed::new_unchecked(delete.clone(), fixt!(Signature));
    let delete_op = Op::RegisterDelete(RegisterDelete {
        delete: delete_signed_hashed.clone(),
        original_action: EntryCreationAction::Create(create.clone()),
        original_entry: None,
    });

    let dna_hash = dna_file.dna_hash().clone();

    // handle only Get events
    let filter_events = |evt: &_| match evt {
        holochain_p2p::event::HolochainP2pEvent::Get { .. } => true,
        _ => false,
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let network = test_network_with_events(
        Some(dna_hash.clone()),
        Some(alice.clone()),
        filter_events,
        tx,
    )
    .await;

    // respond to Get request with action plus delete
    tokio::spawn({
        let action = create_action.clone();
        async move {
            let action_hash = action.clone().to_hash();
            while let Some(evt) = rx.recv().await {
                if let HolochainP2pEvent::Get {
                    dht_hash, respond, ..
                } = evt
                {
                    assert_eq!(dht_hash, action_hash.clone().into());

                    respond.r(ok_fut(Ok(WireOps::Record(WireRecordOps {
                        action: Some(Judged::new(
                            create_action_signed_hashed.clone().into(),
                            ValidationStatus::Valid,
                        )),
                        deletes: vec![Judged::new(
                            WireDelete {
                                delete: delete.clone(),
                                signature: delete_signed_hashed.signature.clone(),
                            },
                            ValidationStatus::Valid,
                        )],
                        updates: vec![],
                        entry: None,
                    }))))
                }
            }
        }
    });

    let workspace_read = get_workspace_read(&test_space, &alice, dna_file.dna_def()).await;
    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback(
        &delete_op,
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![create_action.clone().to_hash().into()]);

    // await while missing record is being fetched in background task
    tokio::time::sleep(Duration::from_millis(500)).await;

    // app validation outcome should be accepted, now that the missing record
    // has been fetched
    let outcome = run_validation_callback(
        &delete_op,
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted)
}

// test that unresolved dependencies of an agent's chain are fetched
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_awaiting_deps_agent_activity() {
    holochain_trace::test_run().unwrap();

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) = op {
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: action.hashed.author().clone(),
                    chain_filter: ChainFilter::new(action.as_hash().clone()),
                });
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(
                            action.hashed.author().clone(),
                            ChainFilter::new(action.as_hash().clone()),
                        ),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        dna_file,
        integrity_zomes,
        test_space,
        ribosome,
    } = TestCase::new(zomes).await;

    let alice = fixt!(AgentPubKey);
    let bob = fixt!(AgentPubKey);

    let action = Action::Dna(Dna {
        author: bob.clone(),
        timestamp: Timestamp::now(),
        hash: fixt!(DnaHash),
    });
    let action_signed_hashed = SignedHashed::new_unchecked(action.clone(), fixt!(Signature));
    let action_op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: action_signed_hashed.clone(),
        cached_entry: None,
    });

    let dna_hash = dna_file.dna_hash().clone();

    let network = test_network(Some(dna_hash.clone()), Some(alice.clone())).await;
    let workspace_read = get_workspace_read(&test_space, &alice, dna_file.dna_def()).await;
    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback(
        &action_op,
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
        fetched_dependencies.clone(),
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
    let outcome = run_validation_callback(
        &action_op,
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}

// test that unresolved dependent hashes are not fetched multiple times
// it cannot be tested for must_get_agent_activity calls, because in this small
// test scenario every agent is an authority and are expected to hold data
// they are an authority of
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_prevent_multiple_identical_fetches() {
    holochain_trace::test_run().unwrap();

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete {
                original_action, ..
            }) = op
            {
                let result = api.must_get_action(MustGetActionInput(original_action.to_hash()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![original_action.to_hash().into()]),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        dna_file,
        integrity_zomes,
        ribosome,
        test_space,
    } = TestCase::new(zomes).await;

    let alice = fixt!(AgentPubKey);
    let bob = fixt!(AgentPubKey);

    // a create
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = Action::Create(create.clone());
    let create_action_signed_hashed =
        SignedHashed::new_unchecked(create_action.clone(), fixt!(Signature));
    let create_action_op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: create_action_signed_hashed.clone(),
        cached_entry: None,
    });

    // a delete that references the create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    let delete_action = Action::Delete(delete.clone());
    let delete_signed_hashed = SignedHashed::new_unchecked(delete.clone(), fixt!(Signature));
    let delete_op = Op::RegisterDelete(RegisterDelete {
        delete: delete_signed_hashed.clone(),
        original_action: EntryCreationAction::Create(create.clone()),
        original_entry: None,
    });

    let dna_hash = dna_file.dna_hash().clone();

    // handle only Get events
    let filter_events = |evt: &_| match evt {
        holochain_p2p::event::HolochainP2pEvent::Get { .. } => true,
        _ => false,
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let network = test_network_with_events(
        Some(dna_file.dna_hash().clone()),
        Some(alice.clone()),
        filter_events,
        tx,
    )
    .await;

    let times_same_hash_is_fetched = Arc::new(AtomicI8::new(0));

    // respond to Get request with requested action
    tokio::spawn({
        let times_fetched = Arc::clone(&times_same_hash_is_fetched);
        async move {
            let action_hash = create_action.clone().to_hash();
            while let Some(evt) = rx.recv().await {
                if let HolochainP2pEvent::Get {
                    dht_hash, respond, ..
                } = evt
                {
                    assert_eq!(dht_hash, action_hash.clone().into());

                    respond.r(ok_fut(Ok(WireOps::Record(WireRecordOps {
                        action: Some(Judged::new(
                            create_action_signed_hashed.clone().into(),
                            ValidationStatus::Valid,
                        )),
                        deletes: vec![Judged::new(
                            WireDelete {
                                delete: delete.clone(),
                                signature: delete_signed_hashed.signature.clone(),
                            },
                            ValidationStatus::Valid,
                        )],
                        updates: vec![],
                        entry: None,
                    }))));

                    times_fetched.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });

    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));
    let workspace_read = get_workspace_read(&test_space, &alice, dna_file.dna_def()).await;

    // run two op validations that depend on the same record in parallel
    let validate_1 = run_validation_callback(
        &delete_op,
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
        fetched_dependencies.clone(),
    );
    let validate_2 = run_validation_callback(
        &delete_op,
        &ribosome,
        workspace_read.clone(),
        network.dna_network(),
        fetched_dependencies.clone(),
    );
    futures::future::join_all([validate_1, validate_2]).await;

    // await while missing records are being fetched in background task
    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(times_same_hash_is_fetched.load(Ordering::Relaxed), 1);
    // after successfully fetching dependencies, the set should be empty
    assert_eq!(fetched_dependencies.lock().missing_hashes.len(), 0);
}

struct TestCase {
    dna_file: DnaFile,
    integrity_zomes: Vec<Zome<IntegrityZomeDef>>,
    test_space: TestSpace,
    ribosome: RealRibosome,
}

impl TestCase {
    async fn new(zomes: SweetInlineZomes) -> Self {
        let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
        let dna_hash = dna_file.dna_hash().clone();
        let ribosome = RealRibosome::new(
            dna_file.clone(),
            Arc::new(RwLock::new(ModuleCache::new(None))),
        )
        .unwrap();
        let test_space = TestSpace::new(dna_hash.clone());
        Self {
            dna_file,
            integrity_zomes,
            test_space,
            ribosome,
        }
    }
}

async fn get_workspace_read(
    test_space: &TestSpace,
    agent_key: &AgentPubKey,
    dna_def: &DnaDef,
) -> HostFnWorkspaceRead {
    HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(agent_key.clone())
            .unwrap()
            .into(),
        test_space.space.dht_db.clone().into(),
        test_space.space.dht_query_cache.clone(),
        test_space.space.cache_db.clone().into(),
        fixt!(MetaLairClient),
        None,
        Arc::new(dna_def.clone()),
    )
    .await
    .unwrap()
}
