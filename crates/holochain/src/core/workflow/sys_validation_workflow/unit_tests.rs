use std::collections::HashSet;
use std::sync::Arc;
use holo_hash::DhtOpHash;
use holo_hash::HasHash;
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::db::DbKindT;
use holochain_sqlite::db::DbWrite;
use holochain_state::mutations::StateMutationResult;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::WireOps;
use holochain_types::record::SignedActionHashedExt;
use holochain_types::record::WireRecordOps;
use holochain_zome_types::Action;
use holochain_zome_types::action::ActionHashed;
use holochain_zome_types::dna_def::{DnaDef, DnaDefHashed};
use holochain_zome_types::judged::Judged;
use holochain_zome_types::record::SignedActionHashed;
use holochain_zome_types::timestamp::Timestamp;
use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::TriggerSender;
use super::SysValidationWorkspace;
use super::sys_validation_workflow;
use fixt::*;
use hdk::prelude::Dna as HdkDna;
use crate::prelude::SignatureFixturator;
use crate::prelude::AgentPubKeyFixturator;
use super::validation_query::get_ops_to_app_validate;
use crate::prelude::CreateFixturator;
use crate::prelude::AgentValidationPkgFixturator;

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_no_dependency() {
    holochain_trace::test_run().unwrap();

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_hash = DnaDefHashed::from_content_sync(dna_def.clone());

    let test_space = TestSpace::new(dna_hash.hash.clone());

    // TODO So this struct is just here to follow the 'workspace' pattern? The Space gets passed to the workflow anyway and most of the fields are shared.
    //      Maybe just moving the Space to the workspace is enough to tidy this up?
    let workspace = SysValidationWorkspace::new(
        test_space.space.authored_db.clone().into(),
        test_space.space.dht_db.clone().into(),
        test_space.space.dht_query_cache.clone(),
        test_space.space.cache_db.clone().into(),
        Arc::new(dna_def.clone()),
    );

    let (app_validation_tx, mut app_validation_rx) = TriggerSender::new();

    let (self_tx, _self_rx) = TriggerSender::new();
    let trigger_self = self_tx.clone();

    let dna_action = HdkDna {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now().into(),
        hash: dna_hash.hash.clone(),
    };
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action));

    let dht_db = test_space.space.dht_db.clone();
    let op_hash = save_op_for_sys_validation(
        dht_db.clone(),
        op,
    ).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_clone().return_once(move || MockHolochainP2pDnaT::new());

    sys_validation_workflow(
        Arc::new(workspace),
        Arc::new(test_space.space),
        app_validation_tx,
        trigger_self,
        network,
    ).await.unwrap();

    let ops_to_app_validate: HashSet<DhtOpHash> = get_ops_to_app_validate(&dht_db.into()).await.unwrap().into_iter().map(|op_hashed| op_hashed.hash).collect();
    assert!(ops_to_app_validate.contains(&op_hash));
    
    tokio::time::timeout(std::time::Duration::from_secs(3), app_validation_rx.listen()).await.expect("Timed out waiting for app validation to be triggered").unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_held_in_cache() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_hash = DnaDefHashed::from_content_sync(dna_def.clone());

    let test_space = TestSpace::new(dna_hash.hash.clone());

    let workspace = SysValidationWorkspace::new(
        test_space.space.authored_db.clone().into(),
        test_space.space.dht_db.clone().into(),
        test_space.space.dht_query_cache.clone(),
        test_space.space.cache_db.clone().into(),
        Arc::new(dna_def.clone()),
    );

    let (app_validation_tx, mut app_validation_rx) = TriggerSender::new();

    let (self_tx, _self_rx) = TriggerSender::new();
    let trigger_self = self_tx.clone();

    let agent = keystore.new_sign_keypair_random().await.unwrap().into();

    // Previous op, to go in the cache
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = agent;
    validation_package_action.action_seq = 10;
    let previous_action = sign_action(&keystore, Action::AgentValidationPkg(validation_package_action.clone()))
        .await;
    let previous_op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::AgentValidationPkg(validation_package_action));
    save_op_for_sys_validation(
        test_space.space.cache_db.clone(),
        previous_op,
    ).await.unwrap();

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let dht_db = test_space.space.dht_db.clone();
    let op_hash = save_op_for_sys_validation(
        dht_db.clone(),
        op,
    ).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    network.expect_clone().return_once(move || MockHolochainP2pDnaT::new());

    sys_validation_workflow(
        Arc::new(workspace),
        Arc::new(test_space.space),
        app_validation_tx,
        trigger_self,
        network,
    ).await.unwrap();

    let ops_to_app_validate: HashSet<DhtOpHash> = get_ops_to_app_validate(&dht_db.into()).await.unwrap().into_iter().map(|op_hashed| op_hashed.hash).collect();
    assert!(ops_to_app_validate.contains(&op_hash));
    
    tokio::time::timeout(std::time::Duration::from_secs(3), app_validation_rx.listen()).await.expect("Timed out waiting for app validation to be triggered").unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_not_held() {
    holochain_trace::test_run().unwrap();

    let keystore = holochain_keystore::test_keystore();

    let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
    let dna_hash = DnaDefHashed::from_content_sync(dna_def.clone());

    let test_space = TestSpace::new(dna_hash.hash.clone());

    let workspace = SysValidationWorkspace::new(
        test_space.space.authored_db.clone().into(),
        test_space.space.dht_db.clone().into(),
        test_space.space.dht_query_cache.clone(),
        test_space.space.cache_db.clone().into(),
        Arc::new(dna_def.clone()),
    );

    let (app_validation_tx, mut app_validation_rx) = TriggerSender::new();

    let (self_tx, _self_rx) = TriggerSender::new();
    let trigger_self = self_tx.clone();

    let agent = keystore.new_sign_keypair_random().await.unwrap().into();

    // Previous op, to be fetched from the network
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = agent;
    validation_package_action.action_seq = 10;
    let previous_action = sign_action(&keystore, Action::AgentValidationPkg(validation_package_action.clone()))
        .await;

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let dht_db = test_space.space.dht_db.clone();
    let op_hash = save_op_for_sys_validation(
        dht_db.clone(),
        op,
    ).await.unwrap();

    let mut network = MockHolochainP2pDnaT::new();

    let mut actual_network = MockHolochainP2pDnaT::new();
    let mut ops: WireRecordOps = WireRecordOps::new();
    ops.action = Some(Judged::valid(previous_action.clone().into()));
    let response = WireOps::Record(ops);
    actual_network.expect_get().return_once(move |_, _| Ok(vec![response]));

    network.expect_clone().return_once(move || actual_network);

    sys_validation_workflow(
        Arc::new(workspace),
        Arc::new(test_space.space),
        app_validation_tx,
        trigger_self,
        network,
    ).await.unwrap();

    let ops_to_app_validate: HashSet<DhtOpHash> = get_ops_to_app_validate(&dht_db.into()).await.unwrap().into_iter().map(|op_hashed| op_hashed.hash).collect();
    assert!(ops_to_app_validate.contains(&op_hash));
    
    tokio::time::timeout(std::time::Duration::from_secs(3), app_validation_rx.listen()).await.expect("Timed out waiting for app validation to be triggered").unwrap();
}

async fn save_op_for_sys_validation<T: DbKindT>(
    vault: DbWrite<T>,
    op: DhtOp,
) -> StateMutationResult<DhtOpHash> {
    let op = DhtOpHashed::from_content_sync(op);

    let test_op_hash = op.as_hash().clone();
    vault
        .write_async({
            move |txn| -> StateMutationResult<()> {
                holochain_state::mutations::insert_op(txn, &op)?;
                Ok(())
            }
        })
        .await
        .unwrap();

    Ok(test_op_hash)
}

pub async fn sign_action(keystore: &MetaLairClient, action: Action) -> SignedActionHashed {
    let action_hashed = ActionHashed::from_content_sync(action);
    SignedActionHashed::sign(keystore, action_hashed)
        .await
        .unwrap()
}
