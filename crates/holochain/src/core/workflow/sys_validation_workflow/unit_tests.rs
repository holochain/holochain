use super::sys_validation_workflow;
use super::validation_deps::ValDeps;
use super::validation_query::get_ops_to_app_validate;
use super::SysValidationWorkspace;
use super::ValidationDependencies;
use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::TriggerReceiver;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use crate::prelude::AgentPubKeyFixturator;
use crate::prelude::AgentValidationPkgFixturator;
use crate::prelude::CreateFixturator;
use crate::prelude::SignatureFixturator;
use fixt::*;
use hdk::prelude::Dna as HdkDna;
use holo_hash::AgentPubKey;
use holo_hash::DhtOpHash;
use holo_hash::DnaHash;
use holo_hash::HasHash;
use holochain_conductor_api::conductor::ConductorConfig;
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_sqlite::db::DbKindCache;
use holochain_sqlite::db::DbKindDht;
use holochain_sqlite::db::DbKindT;
use holochain_sqlite::db::DbWrite;
use holochain_state::mutations::StateMutationResult;
use holochain_types::dht_op::DhtOp;
use holochain_types::dht_op::DhtOpHashed;
use holochain_types::dht_op::WireOps;
use holochain_types::record::SignedActionHashedExt;
use holochain_types::record::WireRecordOps;
use holochain_zome_types::action::ActionHashed;
use holochain_zome_types::action::AppEntryDef;
use holochain_zome_types::action::EntryType;
use holochain_zome_types::dna_def::{DnaDef, DnaDefHashed};
use holochain_zome_types::entry_def::EntryVisibility;
use holochain_zome_types::judged::Judged;
use holochain_zome_types::record::SignedActionHashed;
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
use parking_lot::Mutex;
use std::collections::HashSet;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_no_dependency() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    let dna_action = HdkDna {
        author: fixt!(AgentPubKey),
        timestamp: Timestamp::now().into(),
        hash: test_case.dna_hash(),
    };
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Dna(dna_action));

    let op_hash = test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_held_in_cache() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous op, to go in the cache
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action.clone()))
        .await;
    let previous_op =
        DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(prev_create_action));
    test_case
        .save_op_to_db(test_case.cache_db_handle(), previous_op)
        .await
        .unwrap();

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let op_hash = test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_not_held() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous op, to be fetched from the network
    let mut prev_create_action = fixt!(Create);
    prev_create_action.author = test_case.agent.clone();
    prev_create_action.action_seq = 10;
    prev_create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let previous_action = test_case
        .sign_action(Action::Create(prev_create_action.clone()))
        .await;

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    let op_hash = test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    let mut ops: WireRecordOps = WireRecordOps::new();
    ops.action = Some(Judged::valid(previous_action.clone().into()));
    let response = WireOps::Record(ops);
    network
        .expect_get()
        .return_once(move |_, _| Ok(vec![response]));

    test_case.with_network_behaviour(network).run().await;

    test_case.check_trigger_and_rerun().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_dependency_not_found_on_the_dht() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous op, to be referenced but not found on the dht
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(
            validation_package_action.clone(),
        ))
        .await;

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));

    test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    let mut network = MockHolochainP2pDnaT::new();
    // Just return an empty response, nothing found for the request
    let response = WireOps::Record(WireRecordOps::new());
    network
        .expect_get()
        .return_once(move |_, _| Ok(vec![response]));

    test_case.with_network_behaviour(network).run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.is_empty());

    test_case.expect_app_validation_not_triggered().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn validate_op_with_wrong_sequence_number_rejected_and_not_forwarded_to_app_validation() {
    holochain_trace::test_run().unwrap();

    let mut test_case = TestCase::new().await;

    // Previous op, to be found in the cache
    let mut validation_package_action = fixt!(AgentValidationPkg);
    validation_package_action.author = test_case.agent.clone();
    validation_package_action.action_seq = 10;
    let previous_action = test_case
        .sign_action(Action::AgentValidationPkg(
            validation_package_action.clone(),
        ))
        .await;
    let previous_op = DhtOp::RegisterAgentActivity(
        fixt!(Signature),
        Action::AgentValidationPkg(validation_package_action),
    );
    test_case
        .save_op_to_db(test_case.cache_db_handle(), previous_op)
        .await
        .unwrap();

    // Op to validate, to go in the dht database
    let mut create_action = fixt!(Create);
    create_action.author = previous_action.action().author().clone();
    create_action.action_seq = previous_action.action().action_seq() + 31;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
    create_action.entry_type = EntryType::App(AppEntryDef {
        entry_index: 0.into(),
        zome_index: 0.into(),
        visibility: EntryVisibility::Public,
    });
    let op = DhtOp::RegisterAgentActivity(fixt!(Signature), Action::Create(create_action));
    test_case
        .save_op_to_db(test_case.dht_db_handle(), op)
        .await
        .unwrap();

    test_case.run().await;

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.is_empty());

    test_case.expect_app_validation_not_triggered().await;
}

struct TestCase {
    dna_def: DnaDef,
    dna_hash: DnaDefHashed,
    test_space: TestSpace,
    keystore: MetaLairClient,
    agent: AgentPubKey,
    current_validation_dependencies: ValDeps,
    app_validation_trigger: (TriggerSender, TriggerReceiver),
    self_trigger: (TriggerSender, TriggerReceiver),
    actual_network: Option<MockHolochainP2pDnaT>,
}

impl TestCase {
    async fn new() -> Self {
        let dna_def = DnaDef::unique_from_zomes(vec![], vec![]);
        let dna_hash = DnaDefHashed::from_content_sync(dna_def.clone());

        let test_space = TestSpace::new(dna_hash.hash.clone());

        let keystore = holochain_keystore::test_keystore();
        let agent = keystore.new_sign_keypair_random().await.unwrap().into();

        Self {
            dna_def,
            dna_hash,
            test_space,
            keystore,
            agent,
            current_validation_dependencies: ValDeps::new(),
            app_validation_trigger: TriggerSender::new(),
            self_trigger: TriggerSender::new(),
            actual_network: None,
        }
    }

    fn dna_hash(&self) -> DnaHash {
        self.dna_hash.hash.clone()
    }

    fn dht_db_handle(&self) -> DbWrite<DbKindDht> {
        self.test_space.space.dht_db.clone()
    }

    fn cache_db_handle(&self) -> DbWrite<DbKindCache> {
        self.test_space.space.cache_db.clone()
    }

    async fn sign_action(&self, action: Action) -> SignedActionHashed {
        let action_hashed = ActionHashed::from_content_sync(action);
        SignedActionHashed::sign(&self.keystore, action_hashed)
            .await
            .unwrap()
    }

    fn with_network_behaviour(&mut self, network: MockHolochainP2pDnaT) -> &mut Self {
        self.actual_network = Some(network);
        self
    }

    async fn save_op_to_db<T: DbKindT>(
        &self,
        db: DbWrite<T>,
        op: DhtOp,
    ) -> StateMutationResult<DhtOpHash> {
        let op = DhtOpHashed::from_content_sync(op);

        let test_op_hash = op.as_hash().clone();
        db.write_async({
            move |txn| -> StateMutationResult<()> {
                holochain_state::mutations::insert_op(txn, &op)?;
                Ok(())
            }
        })
        .await
        .unwrap();

        Ok(test_op_hash)
    }

    async fn run(&mut self) -> WorkComplete {
        let workspace = SysValidationWorkspace::new(
            self.test_space.space.authored_db.clone().into(),
            self.test_space.space.dht_db.clone().into(),
            self.test_space.space.dht_query_cache.clone(),
            self.test_space.space.cache_db.clone().into(),
            Arc::new(self.dna_def.clone()),
            None,
            std::time::Duration::from_secs(10),
        );

        let actual_network = self
            .actual_network
            .take()
            .unwrap_or_else(|| MockHolochainP2pDnaT::new());

        // XXX: this isn't quite right, since none of these config settings inform
        // anything else about the TestCase. It's currently only needed for the node_id
        // as used by hc_sleuth
        let config = ConductorConfig::default();
        let config = Arc::new(config);

        sys_validation_workflow(
            Arc::new(workspace),
            self.current_validation_dependencies.clone(),
            self.app_validation_trigger.0.clone(),
            self.self_trigger.0.clone(),
            actual_network,
            config,
        )
        .await
        .unwrap()
    }

    async fn check_trigger_and_rerun(&mut self) -> WorkComplete {
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.self_trigger.1.listen(),
        )
        .await
        .unwrap()
        .unwrap();

        self.run().await
    }

    /// This provides a quick and reliable way to check that ops have been sys validated
    async fn get_ops_pending_app_validation(&self) -> HashSet<DhtOpHash> {
        get_ops_to_app_validate(&self.dht_db_handle().into())
            .await
            .unwrap()
            .into_iter()
            .map(|op_hashed| op_hashed.hash)
            .collect()
    }

    async fn expect_app_validation_triggered(&mut self) {
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.app_validation_trigger.1.listen(),
        )
        .await
        .expect("Timed out waiting for app validation to be triggered")
        .unwrap();
    }

    async fn expect_app_validation_not_triggered(&mut self) {
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(1),
            self.app_validation_trigger.1.listen(),
        )
        .await
        .err()
        .is_some());
    }
}
