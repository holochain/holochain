use crate::core::ribosome::inline_ribosome::{InlineRibosome, InlineZomeStore};
use crate::core::ribosome::Ribosome;
use crate::{
    conductor::space::TestSpace,
    core::{
        ribosome::{guest_callback::validate::ValidateInvocation, ZomesToInvoke},
        workflow::app_validation_workflow::{run_validation_callback, Outcome},
    },
    fixt::MetaLairClientFixturator,
    sweettest::{SweetDnaFile, SweetInlineZomes},
};
use fixt::fixt;
use hdk::prelude::{CreateLinkFixturator, EntryFixturator, RegisterCreateLink};
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::{ActionHash, AgentPubKey, HashableContentExtSync};
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::exports::FallibleIterator;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_types::{
    chain::MustGetAgentActivityResponse,
    db::{DbKindCache, DbWrite},
    dht_op::{ChainOp, DhtOpHashed, WireOps},
    record::WireRecordOps,
};
use holochain_wasm_test_utils::TestWasm;
use holochain_zome_types::{
    chain::{ChainFilter, MustGetAgentActivityInput},
    dependencies::holochain_integrity_types::{UnresolvedDependencies, ValidateCallbackResult},
    entry::MustGetActionInput,
    fixt::{CreateFixturator, DeleteFixturator, SignatureFixturator},
    judged::Judged,
    op::{Op, RegisterAgentActivity, RegisterDelete},
    record::{SignedActionHashed, SignedHashed},
    validate::ValidationStatus,
    Action,
};
use matches::assert_matches;
use std::{sync::Arc, time::Duration};

// test app validation with a must get action where the original action of
// a delete is not in the cache db and then added to it
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_must_get_action() {
    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete { delete }) = op {
                let result =
                    api.must_get_action(MustGetActionInput(delete.hashed.deletes_address.clone()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![delete
                            .hashed
                            .deletes_address
                            .clone()
                            .into()]),
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

    let network = Arc::new(MockHolochainP2pDnaT::new());

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
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();

    // action has not been written to a database yet
    // validation should indicate it is awaiting create action hash
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        false,
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![create_action.to_hash().into()]);

    // Record the action to be must-got during validation into the DhtStore,
    // which the cascade's local read now consults.
    let dht_op = ChainOp::RegisterAgentActivity(fixt!(Signature), create_action.clone());
    let dht_op_hashed = DhtOpHashed::from_content_sync(dht_op);
    test_space
        .space
        .dht_store
        .record_incoming_ops(vec![(dht_op_hashed, false)])
        .await
        .unwrap();

    // the same validation should now successfully validate the op
    let outcome = run_validation_callback(invocation, &ribosome, workspace, network, false)
        .await
        .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}

// same as previous test but this time awaiting the background task that
// fetches the missing original create of a delete
// instead of explicitly writing the missing op to the cache
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_awaiting_deps_hashes() {
    holochain_trace::test_run();

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete { delete }) = op {
                let result =
                    api.must_get_action(MustGetActionInput(delete.hashed.deletes_address.clone()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![delete
                            .hashed
                            .deletes_address
                            .clone()
                            .into()]),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        zomes_to_invoke,
        ribosome,
        alice,
        bob,
        workspace,
        test_space,
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
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();

    // mock network that returns the requested create action
    let mut network = MockHolochainP2pDnaT::new();
    let action_to_return = create_action_signed_hashed.clone();
    network.expect_get().returning(move |hash, _, _| {
        assert_eq!(hash, action_to_return.as_hash().clone().into());
        Ok(vec![WireOps::Record(WireRecordOps {
            action: Some(Judged::new(
                action_to_return.clone().into(),
                ValidationStatus::Valid,
            )),
            deletes: vec![],
            updates: vec![],
            entry: None,
            warrants: vec![],
        })])
    });

    let network = Arc::new(network);

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        false,
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![create_action.clone().to_hash().into()]);

    // await while missing record is being fetched in background task
    await_actions_in_cache(
        &test_space.space.cache_db,
        vec![create_action_signed_hashed.as_hash().clone()],
    )
    .await;

    // The fetched op carries a synthetic signature, so the signature gate keeps
    // it out of the DhtStore (it reaches only the legacy cache, confirmed
    // above). Mirror it into the DhtStore — which the cascade's local read now
    // consults — standing in for the verified fetch a real signature would allow.
    test_space
        .space
        .dht_store
        .record_incoming_ops(vec![(
            DhtOpHashed::from_content_sync(ChainOp::RegisterAgentActivity(
                create_action_signed_hashed.signature().clone(),
                create_action.clone(),
            )),
            false,
        )])
        .await
        .unwrap();

    // app validation outcome should be accepted, now that the missing record
    // has been fetched
    let outcome = run_validation_callback(invocation, &ribosome, workspace, network, false)
        .await
        .unwrap();
    assert_matches!(outcome, Outcome::Accepted)
}

// test that unresolved dependencies of an agent's chain are fetched
#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_awaiting_deps_agent_activity() {
    holochain_trace::test_run();

    let zomes = SweetInlineZomes::new(vec![], 0).integrity_function("validate", {
        move |api, op: Op| {
            if let Op::RegisterDelete(RegisterDelete { delete }) = op {
                // chain filter with delete as chain top and create as chain bottom
                let chain_filter = ChainFilter::until_hash(
                    delete.as_hash().clone(),
                    delete.hashed.deletes_address.clone(),
                );
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: delete.hashed.author.clone(),
                    chain_filter: chain_filter.clone(),
                });
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(
                            delete.hashed.author.clone(),
                            chain_filter.clone(),
                        ),
                    ))
                }
            } else {
                unreachable!()
            }
        }
    });

    let TestCase {
        zomes_to_invoke,
        ribosome,
        alice,
        workspace,
        test_space,
        ..
    } = TestCase::new(zomes).await;

    // a create by alice
    let mut create = fixt!(Create);
    create.author = alice.clone();
    create.action_seq = 0;
    let create_action = Action::Create(create.clone());
    let create_action_signed_hashed =
        SignedActionHashed::new_unchecked(create_action.clone(), fixt!(Signature));
    // a delete by alice that references the create
    let mut delete = fixt!(Delete);
    delete.author = alice.clone();
    delete.action_seq = 1;
    // prev_action must be set, otherwise it will be filtered from the chain
    // that must_get_agent_activity returns
    delete.prev_action = create_action.clone().to_hash();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action = Action::Delete(delete.clone());
    let delete_action_signed_hashed =
        SignedActionHashed::new_unchecked(delete_action.clone(), fixt!(Signature));
    let delete_action_op = Op::RegisterDelete(RegisterDelete {
        delete: SignedHashed::new_unchecked(delete.clone(), fixt!(Signature)),
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &delete_action_op).unwrap();

    let expected_chain_top = delete_action_signed_hashed.clone();

    // mock network with alice not being an authority of bob's action
    let mut network = MockHolochainP2pDnaT::new();
    network.expect_authority_for_hash().returning(|_| Ok(false));
    // return single action as requested chain
    network.expect_must_get_agent_activity().returning({
        let expected_chain_top = expected_chain_top.clone();
        let expected_until_hash = delete.deletes_address.clone();
        let create_action_signed_hashed = create_action_signed_hashed.clone();
        let delete_action_signed_hashed = delete_action_signed_hashed.clone();
        move |author, filter, _, _| {
            assert_eq!(author, alice);
            assert_eq!(&filter.chain_top, expected_chain_top.as_hash());
            assert_eq!(filter.get_until_hash(), Some(&expected_until_hash));

            Ok(vec![MustGetAgentActivityResponse::activity(vec![
                RegisterAgentActivity {
                    action: create_action_signed_hashed.clone(),
                    cached_entry: None,
                },
                RegisterAgentActivity {
                    action: delete_action_signed_hashed.clone(),
                    cached_entry: None,
                },
            ])])
        }
    });
    let network = Arc::new(network);

    // app validation should indicate missing action is being awaited
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        false,
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![expected_chain_top.hashed.author().clone().into()]);

    // await while bob's chain is being fetched in background task
    await_actions_in_cache(
        &test_space.space.cache_db,
        vec![
            create_action_signed_hashed.as_hash().clone(),
            delete_action_signed_hashed.as_hash().clone(),
        ],
    )
    .await;

    // The fetched activity carries synthetic signatures, so the signature gate
    // keeps it out of the DhtStore (it reaches only the legacy cache, confirmed
    // above). Cache bob's chain into the DhtStore as integrated agent activity —
    // which the cascade's local agent-activity read consults — standing in for
    // the verified fetch a real signature would allow.
    let rendered_activity = |sah: &SignedActionHashed| holochain_types::dht_op::RenderedOps {
        entry: None,
        ops: vec![holochain_types::dht_op::RenderedOp::new(
            sah.hashed.content.clone(),
            sah.signature().clone(),
            None,
            holochain_zome_types::op::ChainOpType::RegisterAgentActivity,
        )
        .unwrap()],
        warrant: None,
    };
    test_space
        .space
        .dht_store
        .cache_chain_ops(&rendered_activity(&create_action_signed_hashed))
        .await
        .unwrap();
    test_space
        .space
        .dht_store
        .cache_chain_ops(&rendered_activity(&delete_action_signed_hashed))
        .await
        .unwrap();

    // app validation outcome should be accepted, now that bob's missing agent
    // activity is available in alice's cache
    let outcome = run_validation_callback(invocation, &ribosome, workspace, network, false)
        .await
        .unwrap();
    assert_matches!(outcome, Outcome::Accepted);
}

// An op under validation that depends on an invalid op should be rejected.
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    feature = "wasmer-wasmi",
    ignore = "Waiting for a fix https://github.com/wasmerio/wasmer/issues/6397"
)]
async fn validation_callback_rejects_op_depending_on_invalid_op() {
    holochain_trace::test_run();
    let (dna_file, integrity_zomes, _) =
        SweetDnaFile::unique_from_test_wasms(vec![TestWasm::Link]).await;
    let zomes_to_invoke = ZomesToInvoke::OneIntegrity(integrity_zomes[0].clone());
    let dna_hash = dna_file.dna_hash().clone();
    let ribosome = Ribosome::new_with_test_wasms(vec![TestWasm::Link])
        .await
        .unwrap();
    let test_space = TestSpace::new(dna_hash.clone());
    let alice = fixt!(AgentPubKey);
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(alice.clone())
            .unwrap()
            .into(),
        test_space.space.dht_db.clone().into(),
        test_space.space.dht_store.clone(),
        test_space.space.cache_db.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();

    // An invalid Create action by Alice.
    let mut create = fixt!(Create);
    create.author = alice.clone();
    create.action_seq = 0;
    let create_action = Action::Create(create.clone());
    let create_entry = fixt!(Entry);
    let create_entry_hash = create_action.entry_hash().unwrap().clone();
    // A CreateLink to be validated that does a must_get_valid_record to the invalid Create
    // in the validate callback.
    let mut create_link = fixt!(CreateLink);
    create_link.action_seq = 1;
    create_link.zome_index = 0.into();
    // This link type will lead to a must_get_valid_record in the validate callback.
    create_link.link_type = 2.into();
    create_link.base_address = create_action.to_hash().into();
    let create_link_signed_hashed =
        SignedHashed::new_unchecked(create_link.clone(), fixt!(Signature));
    let create_link_op = Op::RegisterCreateLink(RegisterCreateLink {
        create_link: create_link_signed_hashed,
    });
    let invocation = ValidateInvocation::new(zomes_to_invoke, &create_link_op).unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // Cache the invalid Create record into the DhtStore (integrated, as a
    // fetched op would be) and mark it rejected, so the cascade's
    // get_record_details resolves it as a rejected record.
    let rendered = holochain_types::dht_op::RenderedOp::new(
        create_action.clone(),
        fixt!(Signature),
        None,
        holochain_zome_types::op::ChainOpType::StoreRecord,
    )
    .unwrap();
    let create_op_hash = rendered.op_hash.clone();
    let rendered_ops = holochain_types::dht_op::RenderedOps {
        entry: Some(holochain_types::prelude::EntryHashed::with_pre_hashed(
            create_entry,
            create_entry_hash,
        )),
        ops: vec![rendered],
        warrant: None,
    };
    test_space
        .space
        .dht_store
        .cache_chain_ops(&rendered_ops)
        .await
        .unwrap();
    test_space
        .space
        .dht_store
        .reject_chain_ops(vec![create_op_hash])
        .await
        .unwrap();

    // App validation should reject the CreateLink op because the record at the base address of the link is invalid.
    let outcome = run_validation_callback(
        invocation.clone(),
        &ribosome,
        workspace.clone(),
        network.clone(),
        false,
    )
    .await
    .unwrap();
    assert_matches!(outcome, Outcome::Rejected(reason) if reason == "Found a record, but it is invalid");
}

// test case with alice and bob agent keys
// test space created by alice
struct TestCase {
    zomes_to_invoke: ZomesToInvoke,
    test_space: TestSpace,
    ribosome: Ribosome,
    alice: AgentPubKey,
    bob: AgentPubKey,
    workspace: HostFnWorkspaceRead,
}

impl TestCase {
    async fn new(zomes: SweetInlineZomes) -> Self {
        let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
        let inline_zome_store = InlineZomeStore::default();
        for z in dna_file.inline_zomes() {
            inline_zome_store.insert(dna_file.dna_def_hashed().clone(), z.clone());
        }

        let zomes_to_invoke = ZomesToInvoke::OneIntegrity(integrity_zomes[0].clone());
        let dna_hash = dna_file.dna_hash().clone();
        let ribosome = InlineRibosome::new(dna_file.dna_def_hashed().clone(), inline_zome_store);
        let ribosome = Ribosome::new(dna_file.dna_def_hashed().clone(), ribosome)
            .await
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
            test_space.space.dht_store.clone(),
            test_space.space.cache_db.clone(),
            fixt!(MetaLairClient),
            None,
        )
        .await
        .unwrap();
        Self {
            zomes_to_invoke,
            test_space,
            ribosome,
            alice,
            bob,
            workspace,
        }
    }
}

// wait for provided actions to arrive in cache db
async fn await_actions_in_cache(cache_db: &DbWrite<DbKindCache>, hashes: Vec<ActionHash>) {
    let hashes = Arc::new(hashes.clone());
    loop {
        let hashes = hashes.clone();
        let all_actions_in_cache = cache_db.test_read(move |txn| {
            let mut stmt = txn.prepare("SELECT hash FROM Action").unwrap();
            let rows = stmt.query([]).unwrap();
            let action_hashes_in_cache: Vec<ActionHash> =
                rows.map(|row| row.get(0)).collect().unwrap();
            hashes
                .iter()
                .all(|hash| action_hashes_in_cache.contains(hash))
        });
        if all_actions_in_cache {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
