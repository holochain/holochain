use super::sys_validation_workflow;
use super::validation_query::get_ops_to_app_validate;
use super::SysValidationWorkspace;
use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::TriggerReceiver;
use crate::core::queue_consumer::TriggerSender;
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
use holochain_zome_types::dna_def::{DnaDef, DnaDefHashed};
use holochain_zome_types::judged::Judged;
use holochain_zome_types::record::SignedActionHashed;
use holochain_zome_types::timestamp::Timestamp;
use holochain_zome_types::Action;
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

    let mut network = MockHolochainP2pDnaT::new();
    network
        .expect_clone()
        .return_once(move || MockHolochainP2pDnaT::new());

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
    create_action.action_seq = previous_action.action().action_seq() + 1;
    create_action.prev_action = previous_action.as_hash().clone();
    create_action.timestamp = Timestamp::now().into();
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

    let ops_to_app_validate = test_case.get_ops_pending_app_validation().await;
    assert!(ops_to_app_validate.contains(&op_hash));

    test_case.expect_app_validation_triggered().await;
}

struct TestCase {
    dna_def: DnaDef,
    dna_hash: DnaDefHashed,
    test_space: TestSpace,
    keystore: MetaLairClient,
    agent: AgentPubKey,
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

    async fn run(&mut self) {
        // TODO So this struct is just here to follow the 'workspace' pattern? The Space gets passed to the workflow anyway and most of the fields are shared.
        //      Maybe just moving the Space to the workspace is enough to tidy this up?
        let workspace = SysValidationWorkspace::new(
            self.test_space.space.authored_db.clone().into(),
            self.test_space.space.dht_db.clone().into(),
            self.test_space.space.dht_query_cache.clone(),
            self.test_space.space.cache_db.clone().into(),
            Arc::new(self.dna_def.clone()),
        );

        let mut network = MockHolochainP2pDnaT::new();
        // This can't be copied so if you want to call the run twice you would need to reset the network behaviour!
        let actual_network = self
            .actual_network
            .take()
            .unwrap_or_else(|| MockHolochainP2pDnaT::new());
        network.expect_clone().return_once(move || actual_network);

        sys_validation_workflow(
            Arc::new(workspace),
            Arc::new(self.test_space.space.clone()),
            self.app_validation_trigger.0.clone(),
            self.self_trigger.0.clone(),
            network,
        )
        .await
        .unwrap();
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
}
