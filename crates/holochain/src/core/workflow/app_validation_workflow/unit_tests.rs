use crate::{
    conductor::space::TestSpace,
    core::{
        ribosome::{
            guest_callback::validate::ValidateInvocation, real_ribosome::RealRibosome,
            ZomesToInvoke,
        },
        workflow::app_validation_workflow::{
            run_validation_callback, Outcome, ValidationDependencies,
        },
    },
    fixt::MetaLairClientFixturator,
    sweettest::{SweetDnaFile, SweetInlineZomes},
};
use fixt::fixt;
use holo_hash::{AgentPubKey, HashableContentExtSync};
use holochain_p2p::{HolochainP2pDnaFixturator, MockHolochainP2pDnaT};
use holochain_state::{host_fn_workspace::HostFnWorkspaceRead, mutations::insert_op};
use holochain_types::{
    chain::MustGetAgentActivityResponse,
    dht_op::{DhtOp, DhtOpHashed, WireOps},
    dna::DnaFile,
    record::WireRecordOps,
};
use holochain_wasmer_host::module::ModuleCache;
use holochain_zome_types::{
    action::Delete,
    chain::{ChainFilter, MustGetAgentActivityInput},
    dependencies::holochain_integrity_types::{UnresolvedDependencies, ValidateCallbackResult},
    entry::MustGetActionInput,
    fixt::{AgentPubKeyFixturator, CreateFixturator, DeleteFixturator, SignatureFixturator},
    judged::Judged,
    op::{EntryCreationAction, Op, RegisterAgentActivity, RegisterDelete},
    record::{SignedActionHashed, SignedHashed},
    validate::ValidationStatus,
    Action,
};
use matches::assert_matches;
use parking_lot::{Mutex, RwLock};
use std::{
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
        ribosome,
        test_space,
        workspace,
        zomes_to_invoke,
        alice,
        bob,
        ..
    } = TestCase::new(zomes).await;

    let network = Arc::new(fixt!(HolochainP2pDna));
    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // a create by alice
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = Action::Create(create.clone());
    // a delete by bob that references alice's create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action_signed_hashed = SignedHashed::new_unchecked(delete.clone(), fixt!(Signature));
    let delete_action_op = Op::RegisterDelete(RegisterDelete {
        delete: delete_action_signed_hashed.clone(),
        original_action: EntryCreationAction::Create(create.clone()),
        original_entry: None,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();

    // action has not been written to a database yet
    // validation should indicate it is awaiting create action hash
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
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
        invocation,
        &ribosome,
        workspace,
        network,
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}

// same as previous test but this time awaiting the background task that
// fetches the missing original create of a delete
// instead of explicitly writing the missing op to the cache
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
        zomes_to_invoke,
        ribosome,
        bob,
        workspace,
        ..
    } = TestCase::new(zomes).await;

    // mock network that returns the requested create action
    let mut network = MockHolochainP2pDnaT::new();
    network.expect_get().returning(move |hash, _| {
        assert_eq!(hash, create_action_signed_hashed.as_hash().clone().into());
        Ok(vec![WireOps::Record(WireRecordOps {
            action: Some(Judged::new(
                create_action_signed_hashed.clone().into(),
                ValidationStatus::Valid,
            )),
            deletes: vec![],
            updates: vec![],
            entry: None,
        })])
    });

    // a create by alice
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = Action::Create(create.clone());
    let create_action_signed_hashed =
        SignedHashed::new_unchecked(create_action.clone(), fixt!(Signature));
    // a delete by bob that references alice's create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action_signed_hashed = SignedHashed::new_unchecked(delete.clone(), fixt!(Signature));
    let delete_action_op = Op::RegisterDelete(RegisterDelete {
        delete: delete_action_signed_hashed.clone(),
        original_action: EntryCreationAction::Create(create.clone()),
        original_entry: None,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();
    let network = Arc::new(network);

    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![create_action.clone().to_hash().into()]);

    // await while missing record is being fetched in background task
    tokio::time::sleep(Duration::from_millis(10)).await;

    // app validation outcome should be accepted, now that the missing record
    // has been fetched
    let outcome = run_validation_callback(
        invocation,
        &ribosome,
        workspace,
        network,
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
            if let Op::RegisterDelete(RegisterDelete {
                original_action, ..
            }) = op
            {
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: original_action.author().clone(),
                    chain_filter: ChainFilter::new(original_action.clone().to_hash()),
                });
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(
                            original_action.author().clone(),
                            ChainFilter::new(original_action.clone().to_hash()),
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
        zomes_to_invoke,
        test_space,
        ribosome,
        alice,
        workspace,
        ..
    } = TestCase::new(zomes).await;

    let action_to_be_fetched = create_action_signed_hashed;
    let times_same_hash_is_fetched = Arc::new(AtomicI8::new(0));

    let dna_hash = dna_file.dna_hash().clone();
    let network = test_network(Some(dna_hash.clone()), Some(alice.clone())).await;
    let invocation = ValidateInvocation::new(zomes_to_invoke, &action_op).unwrap();
    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        fetched_dependencies.clone(),
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![action_to_be_fetched.hashed.author().clone().into()]);

    // await while bob's chain is being fetched in background task
    tokio::time::sleep(Duration::from_millis(20)).await;

    // app validation outcome should be accepted, now that bob's missing agent
    // activity is available in alice's cache
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace,
        network,
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
async fn validation_callback_prevent_multiple_identical_hash_fetches() {
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
        zomes_to_invoke,
        ribosome,
        alice,
        bob,
        workspace,
        ..
    } = TestCase::new(zomes).await;

    // a create by alice
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = Action::Create(create.clone());
    let create_action_signed_hashed =
        SignedHashed::new_unchecked(create_action.clone(), fixt!(Signature));
    // a delete by bob that references alice's create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action_signed_hashed = SignedHashed::new_unchecked(delete.clone(), fixt!(Signature));
    let delete_action_op = Op::RegisterDelete(RegisterDelete {
        delete: delete_action_signed_hashed.clone(),
        original_action: EntryCreationAction::Create(create.clone()),
        original_entry: None,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();

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

    // mock network that returns the requested create action and increments
    // counter of identical hash fetches
    let mut network = MockHolochainP2pDnaT::new();
    network.expect_get().returning({
        let times_same_hash_is_fetched = times_same_hash_is_fetched.clone();
        move |hash, _| {
            if hash == action_to_be_fetched.as_hash().clone().into() {
                times_same_hash_is_fetched.fetch_add(1, Ordering::Relaxed);
            };
            Ok(vec![WireOps::Record(WireRecordOps {
                action: Some(Judged::new(
                    action_to_be_fetched.clone().into(),
                    ValidationStatus::Valid,
                )),
                deletes: vec![],
                updates: vec![],
                entry: None,
            })])
        }
    });
    let network = Arc::new(network);

    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // run two op validations that depend on the same record in parallel
    let validate_1 = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        fetched_dependencies.clone(),
    );
    let validate_2 = run_validation_callback(
        invocation,
        &ribosome,
        workspace,
        network,
        fetched_dependencies.clone(),
    );
    futures::future::join_all([validate_1, validate_2]).await;

    // await while missing records are being fetched in background task
    tokio::time::sleep(Duration::from_millis(10)).await;

    assert_eq!(times_same_hash_is_fetched.load(Ordering::Relaxed), 1);
    // after successfully fetching dependencies, the set should be empty
    assert_eq!(fetched_dependencies.lock().missing_hashes.len(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_prevent_multiple_identical_agent_activity_fetches() {
    holochain_trace::test_run().unwrap();

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete {
                original_action, ..
            }) = op
            {
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: original_action.author().clone(),
                    chain_filter: ChainFilter::new(original_action.clone().to_hash()),
                });
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(
                            original_action.author().clone(),
                            ChainFilter::new(original_action.clone().to_hash()),
                        ),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        ribosome,
        alice,
        workspace,
        ..
    } = TestCase::new(zomes).await;

    let action_to_be_fetched = create_action_signed_hashed;
    let times_same_hash_is_fetched = Arc::new(AtomicI8::new(0));

    // mock network with alice not being an authority of bob's action
    let mut network = MockHolochainP2pDnaT::new();
    network.expect_authority_for_hash().returning(|_| Ok(false));
    // return single action as requested chain
    network.expect_must_get_agent_activity().returning({
        let times_same_hash_is_fetched = times_same_hash_is_fetched.clone();
        move |author, filter| {
            assert_eq!(author, alice);
            assert_eq!(&filter.chain_top, action_to_be_fetched.as_hash());

            times_same_hash_is_fetched
                .clone()
                .fetch_add(1, Ordering::Relaxed);
            Ok(vec![MustGetAgentActivityResponse::Activity(vec![
                RegisterAgentActivity {
                    action: action_to_be_fetched.clone(),
                    cached_entry: None,
                },
            ])])
        }
    });
    let network = Arc::new(network);
    let zomes_to_invoke = ZomesToInvoke::OneIntegrity(integrity_zomes[0].clone());
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();

    let fetched_dependencies = Arc::new(Mutex::new(ValidationDependencies::new()));

    // run two op validations that depend on the same record in parallel
    let validate_1 = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        fetched_dependencies.clone(),
    );
    let validate_2 = run_validation_callback(
        invocation,
        &ribosome,
        workspace,
        network,
        fetched_dependencies.clone(),
    );
    futures::future::join_all([validate_1, validate_2]).await;

    // await while missing records are being fetched in background task
    tokio::time::sleep(Duration::from_millis(20)).await;

    assert_eq!(times_same_hash_is_fetched.load(Ordering::Relaxed), 1);
    // after successfully fetching dependencies, the set should be empty
    assert_eq!(fetched_dependencies.lock().missing_hashes.len(), 0);
}

struct TestCase {
    dna_file: DnaFile,
    zomes_to_invoke: ZomesToInvoke,
    test_space: TestSpace,
    ribosome: RealRibosome,
    alice: AgentPubKey,
    bob: AgentPubKey,
    workspace: HostFnWorkspaceRead,
}

impl TestCase {
    async fn new(zomes: SweetInlineZomes) -> Self {
        let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
        let zomes_to_invoke = ZomesToInvoke::OneIntegrity(integrity_zomes[0].clone());
        let dna_hash = dna_file.dna_hash().clone();
        let ribosome = RealRibosome::new(
            dna_file.clone(),
            Arc::new(RwLock::new(ModuleCache::new(None))),
        )
        .unwrap();
        let test_space = TestSpace::new(dna_hash.clone());
        let alice = fixt!(AgentPubKey);
        let bob = fixt!(AgentPubKey);
        let workspace = HostFnWorkspaceRead::new(
            test_space
                .space
                .get_or_create_authored_db(alice.clone())
                .unwrap()
                .into(),
            test_space.space.dht_db.clone().into(),
            test_space.space.dht_query_cache.clone(),
            test_space.space.cache_db.clone().into(),
            fixt!(MetaLairClient),
            None,
            Arc::new(dna_file.dna_def().clone()),
        )
        .await
        .unwrap();
        Self {
            dna_file,
            zomes_to_invoke,
            test_space,
            ribosome,
            alice,
            bob,
            workspace,
        }
    }
}
