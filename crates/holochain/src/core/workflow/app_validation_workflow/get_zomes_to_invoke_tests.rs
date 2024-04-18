use crate::conductor::space::TestSpace;
use crate::core::ribosome::real_ribosome::RealRibosome;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::validation::OutcomeOrError;
use crate::core::workflow::app_validation_workflow::{
    get_zomes_to_invoke, put_validation_limbo, Outcome,
};
use crate::fixt::MetaLairClientFixturator;
use crate::sweettest::{SweetDnaFile, SweetInlineZomes};
use fixt::fixt;
use holo_hash::{HasHash, HashableContentExtSync};
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_state::mutations::insert_op;
use holochain_state::validation_db::ValidationStage;
use holochain_types::dht_op::{DhtOp, DhtOpHashed};
use holochain_types::rate_limit::{EntryRateWeight, RateWeight};
use holochain_zome_types::action::{AppEntryDef, Create, Delete, Dna, EntryType};
use holochain_zome_types::dependencies::holochain_integrity_types::ValidateCallbackResult;
use holochain_zome_types::fixt::{
    ActionFixturator, ActionHashFixturator, AgentPubKeyFixturator, CreateFixturator,
    EntryFixturator, EntryHashFixturator, SignatureFixturator,
};
use holochain_zome_types::op::{Op, RegisterAgentActivity, StoreRecord};
use holochain_zome_types::record::{Record, RecordEntry, SignedActionHashed};
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
use matches::assert_matches;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn get_zomes_to_invoke_register_agent_activity() {
    let zomes = SweetInlineZomes::new(vec![], 0)
        .integrity_function("validate", |_, _: Op| Ok(ValidateCallbackResult::Valid));
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = RealRibosome::empty(dna_file.clone());
    let action = fixt!(Action);
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::RegisterAgentActivity(RegisterAgentActivity {
        action: action.clone(),
        cached_entry: None,
    });
    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(action.hashed.author().clone())
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
    let network = Arc::new(MockHolochainP2pDnaT::new());
    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_zomes_to_invoke_store_record_with_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = RealRibosome::empty(dna_file.clone());
    let entry = fixt!(Entry);
    let action = Action::Create(Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: 0.into(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    });
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });
    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(action.hashed.author().clone())
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
    let network = Arc::new(MockHolochainP2pDnaT::new());
    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(zome) if zome.name == integrity_zomes[0].name);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_zomes_to_invoke_store_record_with_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = RealRibosome::empty(dna_file.clone());
    let action = Action::Create(Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(AgentPubKey).into(),
        entry_type: EntryType::AgentPubKey,
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    });
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });
    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(action.hashed.author().clone())
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
    let network = Arc::new(MockHolochainP2pDnaT::new());
    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn get_zomes_to_invoke_store_record_with_wrong_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = RealRibosome::empty(dna_file.clone());
    let entry = fixt!(Entry);
    let action = Action::Create(Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            // zome with index 1 does not exist
            zome_index: 1.into(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    });
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });
    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(action.hashed.author().clone())
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
    let network = Arc::new(MockHolochainP2pDnaT::new());
    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome).await;
    assert_matches!(
        zomes_to_invoke,
        Err(OutcomeOrError::Outcome(Outcome::Rejected(_)))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn get_zomes_to_invoke_store_record_without_entry_delete() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = RealRibosome::empty(dna_file.clone());
    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: 0.into(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let original_action = Action::Create(create);
    let action = Action::Delete(Delete {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    });
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });
    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(action.hashed.author().clone())
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
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
    test_space.space.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_op).unwrap();
        put_validation_limbo(txn, dht_op.as_hash(), ValidationStage::SysValidated).unwrap();
    });

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(zome) if zome.name == integrity_zomes[0].name);
}
