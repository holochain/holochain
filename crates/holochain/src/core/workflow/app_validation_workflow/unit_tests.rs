use crate::{
    conductor::space::TestSpace,
    core::{
        ribosome::{
            guest_callback::validate::ValidateInvocation, real_ribosome::RealRibosome,
            ZomesToInvoke,
        },
        workflow::app_validation_workflow::{run_validation_callback_inner, Outcome},
    },
    sweettest::{
        SweetConductor, SweetConductorConfig, SweetDnaFile, SweetInlineZomes, SweetLocalRendezvous,
    },
};
use fixt::fixt;
use holo_hash::HashableContentExtSync;
use holochain_keystore::test_keystore;
use holochain_p2p::{
    actor::HolochainP2pRefToDna, spawn_holochain_p2p, stub_network, HolochainP2pDna,
    HolochainP2pDnaT,
};
use holochain_state::{host_fn_workspace::HostFnWorkspaceRead, mutations::insert_op};
use holochain_types::{
    dht_op::{DhtOp, DhtOpHashed},
    inline_zome::InlineZomeSet,
    record::SignedActionHashedExt,
};
use holochain_wasmer_host::module::ModuleCache;
use holochain_zome_types::{
    action::{ActionHashed, AppEntryDef, Create, Delete, EntryType},
    cell::CellId,
    chain::{ChainFilter, MustGetAgentActivityInput},
    dependencies::holochain_integrity_types::{UnresolvedDependencies, ValidateCallbackResult},
    dna_def::{DnaDef, DnaDefHashed},
    entry::{MustGetActionInput, MustGetEntryInput},
    entry_def::EntryVisibility,
    fixt::{CreateFixturator, DeleteFixturator, EntryFixturator, SignatureFixturator},
    op::{Op, RegisterAgentActivity, RegisterDelete, StoreEntry, StoreRecord},
    record::SignedActionHashed,
    Action,
};
use parking_lot::RwLock;
use std::{hash::Hash, sync::Arc};

#[tokio::test(flavor = "multi_thread")]
async fn validation_callback_must_get_action() {
    let zomes =
        SweetInlineZomes::new(vec![], 0).integrity_function("validate", move |api, op: Op| {
            if let Op::RegisterAgentActivity(RegisterAgentActivity {
                action,
                cached_entry,
            }) = op
            {
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

    let test_space = TestSpace::new(dna_file.dna_hash().to_owned());
    let keystore = test_keystore();
    let agent_key = keystore.new_sign_keypair_random().await.unwrap();

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
        test_space.space.authored_db.into(),
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
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.clone(),
    )
    .await
    .unwrap();
    // validation should indicate it is awaiting create action hash
    assert!(
        matches!(outcome, Outcome::AwaitingDeps(hashes) if hashes == vec![create_action.to_hash().into()])
    );

    // write action to be must got during validation to dht cache db
    let dht_op = DhtOp::RegisterAgentActivity(fixt!(Signature), create_action.clone());
    let dht_op_hashed = DhtOpHashed::from_content_sync(dht_op);
    // let cache = conductor.get_cache_db(&cell_id).await.unwrap();
    test_space
        .space
        .cache_db
        .test_write(move |txn| insert_op(txn, &dht_op_hashed))
        .unwrap();

    // the same validation should now be successfully validating the op
    let outcome = run_validation_callback_inner(
        invocation.clone(),
        &ribosome,
        workspace_read.clone(),
        network.clone(),
    )
    .await
    .unwrap();
    assert!(matches!(outcome, Outcome::Accepted));
}

#[tokio::test(flavor = "multi_thread")]
