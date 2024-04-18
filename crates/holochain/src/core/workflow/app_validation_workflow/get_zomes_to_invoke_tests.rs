use crate::conductor::space::TestSpace;
use crate::core::ribosome::{MockRibosomeT, ZomesToInvoke};
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
use holochain_zome_types::action::{
    AppEntryDef, Create, CreateLink, Delete, EntryType, Update, ZomeIndex,
};
use holochain_zome_types::fixt::{
    ActionFixturator, ActionHashFixturator, AgentPubKeyFixturator, CreateFixturator,
    CreateLinkFixturator, DeleteFixturator, DeleteLinkFixturator, EntryFixturator,
    EntryHashFixturator, SignatureFixturator, UpdateFixturator,
};
use holochain_zome_types::op::{
    EntryCreationAction, Op, RegisterAgentActivity, RegisterCreateLink, RegisterDelete,
    RegisterDeleteLink, RegisterUpdate, StoreEntry, StoreRecord,
};
use holochain_zome_types::record::{Record, RecordEntry, SignedActionHashed, SignedHashed};
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
use matches::assert_matches;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn register_agent_activity() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeT::new();

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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn store_entry_create_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let entry = fixt!(Entry);
    let create = Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: zome_index.clone(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = EntryCreationAction::Create(create);
    let action = SignedHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_entry_create_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

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
    let action = EntryCreationAction::Create(create);
    let action = SignedHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn store_entry_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let entry = fixt!(Entry);
    let update = Update {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        entry_hash: entry.to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: zome_index.clone(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: fixt!(ActionHash),
        original_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let action = EntryCreationAction::Update(update);
    let action = SignedHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_entry_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

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
    let action = EntryCreationAction::Update(update);
    let action = SignedHashed::new_unchecked(action, fixt!(Signature));
    let op = Op::StoreEntry(StoreEntry {
        action: action.clone(),
        entry,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn store_record_create_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let entry = fixt!(Entry);
    let action = Action::Create(Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: zome_index.clone(),
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_create_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeT::new();

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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn store_record_create_wrong_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    // zome with index 1 does not exist
    let zome_index = ZomeIndex(1);
    let mut ribosome = MockRibosomeT::new();
    ribosome
        .expect_get_integrity_zome()
        .return_once(move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            None
        });

    let entry = fixt!(Entry);
    let action = Action::Create(Create {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: entry.clone().to_hash(),
        entry_type: EntryType::App(AppEntryDef {
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn store_record_create_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index.clone();
    let action = Action::CreateLink(create_link);
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let action = Action::Update(Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: zome_index.clone(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: create.to_hash(),
        original_entry_address: create.entry_hash.clone(),
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeT::new();

    let mut create = fixt!(Create);
    create.entry_type = EntryType::CapGrant;
    let action = Action::Update(Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::AgentPubKey,
        original_action_address: create.to_hash(),
        original_entry_address: create.entry_hash.clone(),
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn store_record_update_of_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let update = Action::Update(Update {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        entry_hash: fixt!(EntryHash),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: zome_index.clone(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: create.to_hash(),
        original_entry_address: create.entry_hash.clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    });
    let action = Action::Update(Update {
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
    });
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_without_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeT::new();

    let mut create = fixt!(Create);
    create.entry_type = EntryType::CapGrant;
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index.clone();
    let original_action = Action::CreateLink(create_link.clone());
    let mut delete_link = fixt!(DeleteLink);
    delete_link.link_add_address = original_action.to_hash();
    let action = Action::DeleteLink(delete_link);
    let action = SignedActionHashed::new_unchecked(action, fixt!(Signature));
    let record = Record::new(action.clone(), None);
    let op = Op::StoreRecord(StoreRecord { record });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

// not a logical case that a delete is deleted, but valid nonetheless
#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_of_delete_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let delete = Delete {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        deletes_address: create.to_hash(),
        deletes_entry_address: create.entry_hash.clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete_action = Action::Delete(delete.clone());
    let delete_action = SignedActionHashed::new_unchecked(delete_action, fixt!(Signature));
    let action = Action::Delete(Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: delete_action.as_hash().clone(),
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    let dht_op =
        DhtOpHashed::from_content_sync(DhtOp::RegisterDeletedEntryAction(fixt!(Signature), delete));
    test_space.space.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_op).unwrap();
        put_validation_limbo(txn, dht_op.as_hash(), ValidationStage::SysValidated).unwrap();
    });

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

// not a logical case that a delete is deleted, but valid nonetheless
#[tokio::test(flavor = "multi_thread")]
async fn store_record_delete_of_delete_without_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let delete = Delete {
        action_seq: 0,
        author: fixt!(AgentPubKey),
        deletes_address: create.to_hash(),
        deletes_entry_address: create.entry_hash.clone(),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete_action = Action::Delete(delete.clone());
    let delete_action = SignedActionHashed::new_unchecked(delete_action, fixt!(Signature));
    let action = Action::Delete(Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: delete_action.as_hash().clone(),
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
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    let dht_op =
        DhtOpHashed::from_content_sync(DhtOp::RegisterDeletedEntryAction(fixt!(Signature), delete));
    test_space.space.dht_db.test_write(move |txn| {
        insert_op(txn, &dht_op).unwrap();
        put_validation_limbo(txn, dht_op.as_hash(), ValidationStage::SysValidated).unwrap();
    });

    let zomes_to_invoke = get_zomes_to_invoke(&op, &workspace, network, &ribosome)
        .await
        .unwrap();
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let entry = fixt!(Entry);
    let update = Update {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        entry_hash: entry.to_hash(),
        entry_type: EntryType::App(AppEntryDef {
            zome_index: zome_index.clone(),
            entry_index: 0.into(),
            visibility: Default::default(),
        }),
        original_action_address: fixt!(ActionHash),
        original_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: EntryRateWeight::default(),
    };
    let update = SignedHashed::new_unchecked(update, fixt!(Signature));
    let op = Op::RegisterUpdate(RegisterUpdate {
        update: update.clone(),
        new_entry: Some(entry),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeT::new();

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
    let update = SignedHashed::new_unchecked(update, fixt!(Signature));
    let op = Op::RegisterUpdate(RegisterUpdate {
        update: update.clone(),
        new_entry: Some(entry),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
async fn register_delete_create_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let original_action = Action::Create(create);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = SignedHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_create_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create = fixt!(Create);
    create.entry_type = EntryType::CapGrant;
    let original_action = Action::Create(create);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = SignedHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_update_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut update = fixt!(Update);
    update.entry_type = EntryType::App(AppEntryDef {
        zome_index: zome_index.clone(),
        entry_index: 0.into(),
        visibility: Default::default(),
    });
    let original_action = Action::Update(update);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = SignedHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_update_non_app_entry() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut update = fixt!(Update);
    update.entry_type = EntryType::CapClaim;
    let original_action = Action::Update(update);
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = SignedHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

// not a logical case that a delete is deleted, but valid nonetheless
#[tokio::test(flavor = "multi_thread")]
async fn register_delete_of_delete() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, _, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let ribosome = MockRibosomeT::new();

    let original_action = Action::Delete(fixt!(Delete));
    let delete = Delete {
        action_seq: 1,
        author: fixt!(AgentPubKey),
        deletes_address: original_action.to_hash(),
        deletes_entry_address: fixt!(EntryHash),
        prev_action: fixt!(ActionHash),
        timestamp: Timestamp::now(),
        weight: RateWeight::default(),
    };
    let delete = SignedHashed::new_unchecked(delete, fixt!(Signature));
    let op = Op::RegisterDelete(RegisterDelete {
        delete: delete.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    // there is no app entry def in a delete which would indicate the zome index,
    // therefore all integrity zomes are invoked for validation
    assert_matches!(zomes_to_invoke, ZomesToInvoke::AllIntegrity);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_create_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index.clone();
    let create_link = SignedHashed::new_unchecked(create_link, fixt!(Signature));
    let op = Op::RegisterCreateLink(RegisterCreateLink {
        create_link: create_link.clone(),
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}

#[tokio::test(flavor = "multi_thread")]
async fn register_delete_link() {
    let zomes = SweetInlineZomes::new(vec![], 0);
    let (dna_file, integrity_zomes, _) = SweetDnaFile::unique_from_inline_zomes(zomes).await;
    let zome = &integrity_zomes[0];
    let zome_index = ZomeIndex(0);
    let mut ribosome = MockRibosomeT::new();
    ribosome.expect_get_integrity_zome().return_once({
        let zome = zome.clone();
        move |index| {
            assert_eq!(index, &zome_index, "expected zome index {zome_index:?}");
            Some(zome)
        }
    });

    let mut create_link = fixt!(CreateLink);
    create_link.zome_index = zome_index.clone();
    let delete_link = SignedHashed::new_unchecked(fixt!(DeleteLink), fixt!(Signature));
    let op = Op::RegisterDeleteLink(RegisterDeleteLink {
        create_link: create_link.clone(),
        delete_link,
    });

    let test_space = TestSpace::new(dna_file.dna_hash().clone());
    let workspace = HostFnWorkspaceRead::new(
        test_space
            .space
            .get_or_create_authored_db(fixt!(AgentPubKey))
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
    assert_matches!(zomes_to_invoke, ZomesToInvoke::OneIntegrity(z) if z.name == zome.name);
}
