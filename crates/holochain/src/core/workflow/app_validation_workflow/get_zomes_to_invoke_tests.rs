use crate::conductor::space::TestSpace;
use crate::core::ribosome::mock_ribosome::MockRibosomeBuilder;
use crate::core::ribosome::ZomesToInvoke;
use crate::core::validation::OutcomeOrError;
use crate::core::workflow::app_validation_workflow::{get_zomes_to_invoke, Outcome};
use crate::fixt::MetaLairClientFixturator;
use crate::sweettest::{SweetDnaFile, SweetInlineZomes};
use fixt::fixt;
use holo_hash::fixt::{ActionHashFixturator, AgentPubKeyFixturator, EntryHashFixturator};
use holo_hash::HashableContentExtSync;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::host_fn_workspace::HostFnWorkspaceRead;
use holochain_timestamp::Timestamp;
use holochain_types::op::{ChainOp, DhtOp, DhtOpHashed, OpEntry};
use holochain_zome_types::fixt::{
    ActionFixturator, CreateAction, CreateLinkAction, DeleteAction, DeleteLinkAction,
    EntryFixturator, SignatureFixturator, UpdateAction,
};
use holochain_zome_types::prelude::{
    ActionData, AgentActivity, AppEntryDef, CreateEntry, CreateLink, CreateRecord, Delete,
    DeleteLink, EntryType, Op, Record, RecordEntry, SignedAction, SignedActionHashed, Update,
    ZomeIndex,
};
use matches::assert_matches;
use std::sync::Arc;

/// Seed a dependency op into the `DhtStore`.
///
/// `get_zomes_to_invoke` resolves the original action via the cascade, whose
/// local read is `DhtStore`-backed, so a dependency op must be recorded into
/// the store to be resolvable.
async fn seed_dependency_op(test_space: &TestSpace, dht_op: DhtOpHashed) {
    test_space
        .space
        .dht_store
        // For this op, a validation receipt should not be requested.
        .record_incoming_ops(vec![(dht_op, false)])
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn register_agent_activity() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

    let action = fixt!(Action);
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::AgentActivity(AgentActivity {
        action: action.clone(),
        cached_entry: None,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn store_entry_create_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut action = fixt!(Action, CreateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = entry.clone().to_hash();
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::CreateEntry(CreateEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_entry_create_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut action = fixt!(Action, CreateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = entry.clone().to_hash();
    *action.entry_type_mut().unwrap() = EntryType::CapClaim;
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::CreateEntry(CreateEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn store_entry_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut action = fixt!(Action, UpdateAction);
    action.header.action_seq = 1;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = entry.to_hash();
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    if let ActionData::Update(d) = &mut action.data {
        d.original_action_address = fixt!(ActionHash);
        d.original_entry_address = fixt!(EntryHash);
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::CreateEntry(CreateEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_entry_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut action = fixt!(Action, UpdateAction);
    action.header.action_seq = 1;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = entry.to_hash();
    *action.entry_type_mut().unwrap() = EntryType::AgentPubKey;
    if let ActionData::Update(d) = &mut action.data {
        d.original_action_address = fixt!(ActionHash);
        d.original_entry_address = fixt!(EntryHash);
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::CreateEntry(CreateEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn store_record_create_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut action = fixt!(Action, CreateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = entry.clone().to_hash();
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_create_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

    let mut action = fixt!(Action, CreateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = fixt!(AgentPubKey).into();
    *action.entry_type_mut().unwrap() = EntryType::AgentPubKey;
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn store_record_create_wrong_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut action = fixt!(Action, CreateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = entry.clone().to_hash();
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        // zome with index 1 does not exist
        zome_index: 1.into(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn store_record_create_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut action = fixt!(Action, CreateLinkAction);
    if let ActionData::CreateLink(d) = &mut action.data {
        d.zome_index = zome_index;
    }
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut create = fixt!(Action, CreateAction);
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let mut action = fixt!(Action, UpdateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    if let ActionData::Update(d) = &mut action.data {
        d.original_action_address = create.to_hash();
        d.original_entry_address = create.entry_hash().unwrap().clone();
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

    let mut create = fixt!(Action, CreateAction);
    *create.entry_type_mut().unwrap() = EntryType::CapGrant;
    let mut action = fixt!(Action, UpdateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    *action.entry_type_mut().unwrap() = EntryType::AgentPubKey;
    if let ActionData::Update(d) = &mut action.data {
        d.original_action_address = create.to_hash();
        d.original_entry_address = create.entry_hash().unwrap().clone();
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn store_record_update_of_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut create = fixt!(Action, CreateAction);
    *create.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let mut update = fixt!(Action, UpdateAction);
    update.header.action_seq = 0;
    update.header.author = fixt!(AgentPubKey);
    *update.entry_hash_mut().unwrap() = fixt!(EntryHash);
    *update.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    if let ActionData::Update(d) = &mut update.data {
        d.original_action_address = create.to_hash();
        d.original_entry_address = create.entry_hash().unwrap().clone();
    }
    update.header.prev_action = Some(fixt!(ActionHash));
    update.header.timestamp = Timestamp::now();
    let mut action = fixt!(Action, UpdateAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    *action.entry_hash_mut().unwrap() = fixt!(EntryHash);
    *action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index: 0.into(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    if let ActionData::Update(d) = &mut action.data {
        d.original_action_address = update.to_hash();
        d.original_entry_address = update.entry_hash().unwrap().clone();
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_without_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut original_action = fixt!(Action, CreateAction);
    *original_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let mut action = fixt!(Action, DeleteAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    if let ActionData::Delete(d) = &mut action.data {
        d.deletes_address = original_action.to_hash();
        d.deletes_entry_address = fixt!(EntryHash);
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

    let mut original_action = fixt!(Action, CreateAction);
    *original_action.entry_type_mut().unwrap() = EntryType::CapGrant;
    let mut action = fixt!(Action, DeleteAction);
    action.header.action_seq = 0;
    action.header.author = fixt!(AgentPubKey);
    if let ActionData::Delete(d) = &mut action.data {
        d.deletes_address = original_action.to_hash();
        d.deletes_entry_address = fixt!(EntryHash);
    }
    action.header.prev_action = Some(fixt!(ActionHash));
    action.header.timestamp = Timestamp::now();
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut original_action = fixt!(Action, CreateLinkAction);
    if let ActionData::CreateLink(d) = &mut original_action.data {
        d.zome_index = zome_index;
    }
    let mut action = fixt!(Action, DeleteLinkAction);
    if let ActionData::DeleteLink(d) = &mut action.data {
        d.link_add_address = original_action.to_hash();
    }
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::CreateRecord(CreateRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let entry = fixt!(Entry);
    let mut update = fixt!(Action, UpdateAction);
    update.header.action_seq = 1;
    update.header.author = fixt!(AgentPubKey);
    *update.entry_hash_mut().unwrap() = entry.to_hash();
    *update.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    if let ActionData::Update(d) = &mut update.data {
        d.original_action_address = fixt!(ActionHash);
        d.original_entry_address = fixt!(EntryHash);
    }
    update.header.prev_action = Some(fixt!(ActionHash));
    update.header.timestamp = Timestamp::now();
    let update = SignedActionHashed::new_unchecked(update, fixt!(Signature));
    let op = Op::Update(Update {
        update: update.clone(),
        new_entry: Some(entry),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new().build().await.unwrap();

    let entry = fixt!(Entry);
    let mut update = fixt!(Action, UpdateAction);
    update.header.action_seq = 1;
    update.header.author = fixt!(AgentPubKey);
    *update.entry_hash_mut().unwrap() = entry.to_hash();
    *update.entry_type_mut().unwrap() = EntryType::CapClaim;
    if let ActionData::Update(d) = &mut update.data {
        d.original_action_address = fixt!(ActionHash);
        d.original_entry_address = fixt!(EntryHash);
    }
    update.header.prev_action = Some(fixt!(ActionHash));
    update.header.timestamp = Timestamp::now();
    let update = SignedActionHashed::new_unchecked(update, fixt!(Signature));
    let op = Op::Update(Update {
        update: update.clone(),
        new_entry: Some(entry),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
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
async fn register_delete_create_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut original_action = fixt!(Action, CreateAction);
    *original_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let mut delete = fixt!(Action, DeleteAction);
    delete.header.action_seq = 1;
    delete.header.author = fixt!(AgentPubKey);
    if let ActionData::Delete(d) = &mut delete.data {
        d.deletes_address = original_action.to_hash();
        d.deletes_entry_address = fixt!(EntryHash);
    }
    delete.header.prev_action = Some(fixt!(ActionHash));
    delete.header.timestamp = Timestamp::now();
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::Delete(Delete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_create_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut original_action = fixt!(Action, CreateAction);
    *original_action.entry_type_mut().unwrap() = EntryType::CapGrant;
    let mut delete = fixt!(Action, DeleteAction);
    delete.header.action_seq = 1;
    delete.header.author = fixt!(AgentPubKey);
    if let ActionData::Delete(d) = &mut delete.data {
        d.deletes_address = original_action.to_hash();
        d.deletes_entry_address = fixt!(EntryHash);
    }
    delete.header.prev_action = Some(fixt!(ActionHash));
    delete.header.timestamp = Timestamp::now();
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::Delete(Delete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut original_action = fixt!(Action, UpdateAction);
    *original_action.entry_type_mut().unwrap() = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let mut delete = fixt!(Action, DeleteAction);
    delete.header.action_seq = 1;
    delete.header.author = fixt!(AgentPubKey);
    if let ActionData::Delete(d) = &mut delete.data {
        d.deletes_address = original_action.to_hash();
        d.deletes_entry_address = fixt!(EntryHash);
    }
    delete.header.prev_action = Some(fixt!(ActionHash));
    delete.header.timestamp = Timestamp::now();
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::Delete(Delete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut original_action = fixt!(Action, UpdateAction);
    *original_action.entry_type_mut().unwrap() = EntryType::CapClaim;
    let mut delete = fixt!(Action, DeleteAction);
    delete.header.action_seq = 1;
    delete.header.author = fixt!(AgentPubKey);
    if let ActionData::Delete(d) = &mut delete.data {
        d.deletes_address = original_action.to_hash();
        d.deletes_entry_address = fixt!(EntryHash);
    }
    delete.header.prev_action = Some(fixt!(ActionHash));
    delete.header.timestamp = Timestamp::now();
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::Delete(Delete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    // write original action to dht db
    let dht_op = DhtOpHashed::from_content_sync(DhtOp::from(ChainOp::CreateRecord(
        SignedAction::new(original_action.clone(), fixt!(Signature)),
        OpEntry::ActionOnly,
    )));
    seed_dependency_op(&test_space, dht_op).await;

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_create_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut create_link = fixt!(Action, CreateLinkAction);
    if let ActionData::CreateLink(d) = &mut create_link.data {
        d.zome_index = zome_index;
    }
    let create_link = SignedActionHashed::new_unchecked(create_link, fixt!(Signature));
    let op = Op::CreateLink(CreateLink {
        create_link: create_link.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_file.dna_def_hashed().clone())
        .build()
        .await
        .unwrap();

    let mut create_link = fixt!(Action, CreateLinkAction);
    if let ActionData::CreateLink(d) = &mut create_link.data {
        d.zome_index = zome_index;
    }
    let delete_link = fixt!(Action, DeleteLinkAction);
    let delete_link = SignedActionHashed::new_unchecked(delete_link, fixt!(Signature));
    let op = Op::DeleteLink(DeleteLink {
        create_link,
        delete_link,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space.space.dht_store.clone(),
        fixt!(MetaLairClient),
        None,
    )
    .await
    .unwrap();
    let network = Arc::new(MockHolochainP2pDnaT::new());

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}
