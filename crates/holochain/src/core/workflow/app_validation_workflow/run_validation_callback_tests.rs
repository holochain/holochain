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
use hdk::prelude::{CreateLinkFixturator, EntryFixturator};
use holo_hash::fixt::AgentPubKeyFixturator;
use holo_hash::{ActionHash, AgentPubKey, HashableContentExtSync};
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_types::{
    chain::MustGetAgentActivityResponse,
    dht_op::{ChainOp, DhtOpHashed, WireOps},
    prelude::SignedActionHashedExt,
    record::WireRecordOps,
};
use holochain_wasm_test_utils::TestWasm;
// The wasm `validate(op: Op)` callback decodes the v2 `Op`; `LegacyAction` is
// the per-variant enum the fixturated `Create`/`Delete`/`CreateLink` structs
// plug into before being projected to the v2 `Action` via `from_legacy_action`.
// `MustGetAgentActivityResponse` (unlike the `Op`/`Record` model) still
// carries its activity as legacy `RegisterAgentActivity`/`SignedActionHashed`
// (see `crate::core::ribosome::host_fn::must_get_agent_activity`), so that one
// stays imported from the legacy `op` module and is bridged with
// `to_legacy_signed_action`.
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action as LegacyAction;
use holochain_zome_types::dependencies::holochain_integrity_types::dht_v2::{
    ActionData, DeleteData, Op, RegisterCreateLink, RegisterDelete,
};
use holochain_zome_types::dht_v2::{from_legacy_action, to_legacy_signed_action, SignedAction};
use holochain_zome_types::{
    chain::{ChainFilter, MustGetAgentActivityInput},
    dependencies::holochain_integrity_types::{UnresolvedDependencies, ValidateCallbackResult},
    entry::MustGetActionInput,
    fixt::{CreateFixturator, DeleteFixturator, SignatureFixturator},
    judged::Judged,
    op::RegisterAgentActivity,
    record::SignedActionHashed,
    validate::ValidationStatus,
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
                let deletes_address = match &delete.hashed.content.data {
                    ActionData::Delete(DeleteData {
                        deletes_address, ..
                    }) => deletes_address.clone(),
                    _ => unreachable!(),
                };
                let result = api.must_get_action(MustGetActionInput(deletes_address.clone()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![deletes_address.into()]),
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
    let create_action = LegacyAction::Create(create.clone());
    // a delete by bob that references alice's create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action_v2 = from_legacy_action(&LegacyAction::Delete(delete.clone()));
    let delete_action_signed_hashed =
        SignedActionHashed::new_unchecked(delete_action_v2, fixt!(Signature));
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
    // `dht_op_hashed` is legacy (see the `holochain_types::dht_op::{ChainOp,
    // DhtOpHashed, WireOps}` import above); `record_incoming_ops` is
    // v2-native, so project it at this boundary via `from_legacy_dht_op`.
    test_space
        .space
        .dht_store
        .record_incoming_ops(vec![(
            holochain_types::dht_v2::from_legacy_dht_op(&dht_op_hashed),
            false,
        )])
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
                let deletes_address = match &delete.hashed.content.data {
                    ActionData::Delete(DeleteData {
                        deletes_address, ..
                    }) => deletes_address.clone(),
                    _ => unreachable!(),
                };
                let result = api.must_get_action(MustGetActionInput(deletes_address.clone()));
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::Hashes(vec![deletes_address.into()]),
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
        keystore,
        alice,
        bob,
        workspace,
        test_space,
    } = TestCase::new(zomes).await;

    // a create by alice, signed with alice's real key
    let mut create = fixt!(Create);
    create.author = alice.clone();
    let create_action = LegacyAction::Create(create.clone());
    let create_action_signed_hashed =
        SignedActionHashed::sign(&keystore, from_legacy_action(&create_action).into_hashed())
            .await
            .unwrap();
    // a delete by bob that references alice's create
    let mut delete = fixt!(Delete);
    delete.author = bob.clone();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action_signed_hashed = SignedActionHashed::new_unchecked(
        from_legacy_action(&LegacyAction::Delete(delete.clone())),
        fixt!(Signature),
    );
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
                SignedAction::new(
                    action_to_return.hashed.content.clone(),
                    action_to_return.signature().clone(),
                ),
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

    // The fetched create carries alice's real signature, so it passes the
    // signature gate and lands in the DhtStore, which the cascade's local read
    // consults. Wait for the background fetch to store it.
    await_action_in_store(&test_space.space.dht_store, &create_action.to_hash()).await;

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
                let deletes_address = match &delete.hashed.content.data {
                    ActionData::Delete(DeleteData {
                        deletes_address, ..
                    }) => deletes_address.clone(),
                    _ => unreachable!(),
                };
                let author = delete.hashed.content.author().clone();
                // chain filter with delete as chain top and create as chain bottom
                let chain_filter =
                    ChainFilter::until_hash(delete.as_hash().clone(), deletes_address);
                let result = api.must_get_agent_activity(MustGetAgentActivityInput {
                    author: author.clone(),
                    chain_filter: chain_filter.clone(),
                });
                if result.is_ok() {
                    Ok(ValidateCallbackResult::Valid)
                } else {
                    Ok(ValidateCallbackResult::UnresolvedDependencies(
                        UnresolvedDependencies::AgentActivity(author, chain_filter),
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
        keystore,
        alice,
        workspace,
        test_space,
        ..
    } = TestCase::new(zomes).await;

    // a create by alice, signed with alice's real key
    let mut create = fixt!(Create);
    create.author = alice.clone();
    create.action_seq = 0;
    let create_action = LegacyAction::Create(create.clone());
    let create_action_signed_hashed =
        SignedActionHashed::sign(&keystore, from_legacy_action(&create_action).into_hashed())
            .await
            .unwrap();
    // a delete by alice that references the create
    let mut delete = fixt!(Delete);
    delete.author = alice.clone();
    delete.action_seq = 1;
    // prev_action must be set, otherwise it will be filtered from the chain
    // that must_get_agent_activity returns
    delete.prev_action = create_action.clone().to_hash();
    delete.deletes_address = create_action.clone().to_hash();
    let delete_action_v2 = from_legacy_action(&LegacyAction::Delete(delete.clone()));
    let delete_action_signed_hashed =
        SignedActionHashed::sign(&keystore, delete_action_v2.clone().into_hashed())
            .await
            .unwrap();
    let delete_action_op = Op::RegisterDelete(RegisterDelete {
        delete: SignedActionHashed::new_unchecked(delete_action_v2, fixt!(Signature)),
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
                    action: to_legacy_signed_action(&create_action_signed_hashed),
                    cached_entry: None,
                },
                RegisterAgentActivity {
                    action: to_legacy_signed_action(&delete_action_signed_hashed),
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

    // The fetched activity carries alice's real signatures, so it passes the
    // signature gate and lands in the DhtStore. Wait for the background fetch to
    // store the chain.
    await_action_in_store(
        &test_space.space.dht_store,
        create_action_signed_hashed.as_hash(),
    )
    .await;
    await_action_in_store(
        &test_space.space.dht_store,
        delete_action_signed_hashed.as_hash(),
    )
    .await;

    // app validation outcome should be accepted, now that bob's missing agent
    // activity is available in the DhtStore
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
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();

    // An invalid Create action by Alice.
    let mut create = fixt!(Create);
    create.author = alice.clone();
    create.action_seq = 0;
    let create_action = from_legacy_action(&LegacyAction::Create(create.clone()));
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
    let create_link_action = from_legacy_action(&LegacyAction::CreateLink(create_link));
    let create_link_signed_hashed =
        SignedActionHashed::new_unchecked(create_link_action, fixt!(Signature));
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
    keystore: MetaLairClient,
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
        // Real keypairs so fetched ops carry verifiable signatures and land in
        // the DhtStore, the source every cascade read resolves against.
        let keystore = holochain_keystore::test_keystore();
        let alice = keystore.new_sign_keypair_random().await.unwrap();
        let bob = keystore.new_sign_keypair_random().await.unwrap();
        let workspace =
            HostFnWorkspaceRead::new(test_space.space.dht_store.clone(), keystore.clone(), None)
                .await
                .unwrap();
        Self {
            zomes_to_invoke,
            test_space,
            ribosome,
            keystore,
            alice,
            bob,
            workspace,
        }
    }
}

// Wait for the given action to be fetched into the DhtStore.
async fn await_action_in_store(
    dht_store: &holochain_state::dht_store::DhtStore,
    hash: &ActionHash,
) {
    loop {
        if dht_store
            .as_read()
            .retrieve_action(hash)
            .await
            .unwrap()
            .is_some()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
