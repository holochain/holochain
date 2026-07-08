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
use holochain_types::dht_op::{ChainOp, DhtOpHashed};
use holochain_types::rate_limit::{EntryRateWeight, RateWeight};
use holochain_zome_types::action::{AppEntryDef, Create, Delete, EntryType, Update, ZomeIndex};
// `get_zomes_to_invoke` dispatches on the v2 `Op`; the bare `Op`/`Record`
// names otherwise resolve ambiguously since no legacy `op`/`Action` glob is
// imported here. `LegacyAction` is the per-variant enum the fixturated
// `Create`/`Update`/`Delete`/`CreateLink`/`DeleteLink` structs plug into
// before being projected to the v2 `Action` via `from_legacy_action`.
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action as LegacyAction;
use holochain_zome_types::dependencies::holochain_integrity_types::dht_v2::{
    from_legacy_action, Op, RegisterAgentActivity, RegisterCreateLink, RegisterDelete,
    RegisterDeleteLink, RegisterUpdate, StoreEntry, StoreRecord,
};
use holochain_zome_types::fixt::{
    ActionFixturator, CreateFixturator, CreateLinkFixturator, DeleteLinkFixturator,
    EntryFixturator, SignatureFixturator, UpdateFixturator,
};
use holochain_zome_types::record::{Record, RecordEntry, SignedActionHashed};
use holochain_zome_types::timestamp::Timestamp;
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
        // For this op, a validation receipt should not be requested. `dht_op`
        // is legacy (see the `holochain_types::dht_op::{ChainOp, DhtOpHashed}`
        // import above); `record_incoming_ops` is v2-native, so project it at
        // this boundary via `from_legacy_dht_op`.
        .record_incoming_ops(vec![(
            holochain_types::dht_v2::from_legacy_dht_op(&dht_op),
            false,
        )])
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
    let op = Op::RegisterAgentActivity(RegisterAgentActivity {
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
    let create = Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index,
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Create(create));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
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
    let create = Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::CapClaim,
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Create(create));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
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
    let update = Update {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        entry_hash: entry.to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index,
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: fixt!(ActionHash),
        original_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Update(update));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
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
    let update = Update {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        entry_hash: entry.to_hash(),
        entry_type: EntryType::AgentPubKey,
        original_action_address: fixt!(ActionHash),
        original_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Update(update));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
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
    let create = Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index,
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Create(create));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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

    let create = Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(AgentPubKey).into(),
        entry_type: EntryType::AgentPubKey,
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Create(create));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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
    let create = Create {
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
    };
    let action = from_legacy_action(&LegacyAction::Create(create));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index;
    let action = from_legacy_action(&LegacyAction::CreateLink(create_link));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let update = Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::App(AppEntryDef {
            zome_index,
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: create.to_hash(),
        original_entry_address: create.entry_hash.clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Update(update));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::CapGrant;
    let update = Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::AgentPubKey,
        original_action_address: create.to_hash(),
        original_entry_address: create.entry_hash.clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Update(update));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let update = LegacyAction::Update(Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::App(AppEntryDef {
            zome_index,
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: create.to_hash(),
        original_entry_address: create.entry_hash.clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    });
    let update_of_update = Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: 0.into(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: update.to_hash(),
        original_entry_address: update.entry_hash().unwrap().clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Update(update_of_update));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let original_action = LegacyAction::Create(create);
    let delete = Delete {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Delete(delete));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::CapGrant;
    let original_action = LegacyAction::Create(create);
    let delete = Delete {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let action = from_legacy_action(&LegacyAction::Delete(delete));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index;
    let original_action = LegacyAction::CreateLink(create_link.clone());
    let mut delete_link = fixt!(DeleteLink);
    delete_link.link_add_address = original_action.to_hash();
    let action = from_legacy_action(&LegacyAction::DeleteLink(delete_link));
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(
        action.clone(),
        RecordEntry::new(action.hashed.content.entry_visibility(), None),
    );
    let op = Op::StoreRecord(StoreRecord { record });

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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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
    let update = Update {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        entry_hash: entry.to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index,
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: fixt!(ActionHash),
        original_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let update = from_legacy_action(&LegacyAction::Update(update));
    let update = SignedActionHashed::new_unchecked(update, fixt!(Signature));
    let op = Op::RegisterUpdate(RegisterUpdate {
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
    let update = Update {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        entry_hash: entry.to_hash(),
        entry_type: EntryType::CapClaim,
        original_action_address: fixt!(ActionHash),
        original_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let update = from_legacy_action(&LegacyAction::Update(update));
    let update = SignedActionHashed::new_unchecked(update, fixt!(Signature));
    let op = Op::RegisterUpdate(RegisterUpdate {
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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let original_action = LegacyAction::Create(create);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = from_legacy_action(&LegacyAction::Delete(delete));
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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

    let mut create = fixt!(Create);
    create.entry_type = EntryType::CapGrant;
    let original_action = LegacyAction::Create(create);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = from_legacy_action(&LegacyAction::Delete(delete));
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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

    let mut update = fixt!(Update);
    update.entry_type = EntryType::App(AppEntryDef {
        zome_index,
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let original_action = LegacyAction::Update(update);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = from_legacy_action(&LegacyAction::Delete(delete));
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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

    let mut update = fixt!(Update);
    update.entry_type = EntryType::CapClaim;
    let original_action = LegacyAction::Update(update);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = from_legacy_action(&LegacyAction::Delete(delete));
    let delete = SignedActionHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
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
    let dht_op = DhtOpHashed::from_content_sync(ChainOp::StoreRecord(
        fixt!(Signature),
        original_action,
        RecordEntry::NA,
    ));
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

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index;
    let create_link = from_legacy_action(&LegacyAction::CreateLink(create_link));
    let create_link = SignedActionHashed::new_unchecked(create_link, fixt!(Signature));
    let op = Op::RegisterCreateLink(RegisterCreateLink {
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

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index;
    let create_link = from_legacy_action(&LegacyAction::CreateLink(create_link));
    let delete_link = from_legacy_action(&LegacyAction::DeleteLink(fixt!(DeleteLink)));
    let delete_link = SignedActionHashed::new_unchecked(delete_link, fixt!(Signature));
    let op = Op::RegisterDeleteLink(RegisterDeleteLink {
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
