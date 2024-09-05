use crate::conductor::space::TestSpace;
use crate::core::queue_consumer::{TriggerReceiver, TriggerSender};
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::countersigning_workflow::{
    accept_countersigning_request, countersigning_workflow, CountersigningSessionState,
    SessionCompletionDecision, SessionResolutionSummary,
};
use crate::core::workflow::countersigning_workflow::{countersigning_success, WorkComplete};
use crate::core::workflow::WorkflowResult;
use crate::prelude::CreateFixturator;
use crate::prelude::EntryFixturator;
use crate::prelude::SignatureFixturator;
use crate::prelude::SignedAction;
use crate::prelude::{ActionBase, PreflightBytes, PreflightRequest, PreflightRequestAcceptance};
use crate::prelude::{ActionHashed, CounterSigningAgentState, DhtDbQueryCache, SignedActionHashed};
use fixt::prelude::*;
use hdk::prelude::{Action, Entry, EntryTypeFixturator, Record};
use hdk::prelude::{CounterSigningSessionTimes, Timestamp};
use holo_hash::fixt::ActionHashFixturator;
use holo_hash::fixt::DnaHashFixturator;
use holo_hash::fixt::EntryHashFixturator;
use holo_hash::{AgentPubKey, DnaHash, EntryHash};
use holochain_keystore::MetaLairClient;
use holochain_p2p::MockHolochainP2pDnaT;
use holochain_state::chain_lock::get_chain_lock;
use holochain_state::prelude::AppEntryBytesFixturator;
use holochain_state::prelude::StateMutationResult;
use holochain_state::prelude::{
    insert_action, insert_entry, insert_op, unlock_chain, CounterSigningSessionData,
};
use holochain_state::source_chain;
use holochain_types::activity::AgentActivityResponse;
use holochain_types::dht_op::{ChainOp, DhtOp, DhtOpHashed};
use holochain_types::prelude::SystemSignal;
use holochain_types::prelude::{ChainItems, SignedActionHashedExt};
use holochain_types::signal::Signal;
use holochain_zome_types::cell::CellId;
use holochain_zome_types::countersigning::PreflightResponse;
use holochain_zome_types::prelude::CreateBase;
use holochain_zome_types::query::{ChainHead, ChainStatus};
use matches::assert_matches;
use std::ops::Add;
use std::sync::Arc;
use tokio::sync::broadcast::{Receiver, Sender};

#[tokio::test(flavor = "multi_thread")]
async fn accept_countersigning_request_creates_state() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(60), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn countersigning_session_expiry() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Accept should have triggered the workflow, respond to that run
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // State shouldn't change, just a callback registered to trigger the workflow on expiry
    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;
    test_harness.expect_no_pending_signals();

    // Wait for the workflow to run itself again on expiry
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;
    test_harness.expect_abandoned_signal().await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
}

#[tokio::test(flavor = "multi_thread")]
async fn chain_unlocked_outside_workflow() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Simulate what would happen on a failed commit, the chain gets unlocked and the countersigning
    // workflow must be triggered
    test_harness.unlock_chain().await;
    test_harness.countersigning_tx.trigger(&"test");

    // The refresh mechanism should spot the missing chain lock
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // and terminate the session
    test_harness.expect_abandoned_signal().await;

    test_harness.expect_empty_workspace();
    test_harness.expect_no_pending_signals();
}

#[tokio::test(flavor = "multi_thread")]
async fn discard_session_with_lock_but_no_state() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    test_harness
        .accept_countersigning_request(request)
        .await
        .unwrap();

    test_harness.expect_session_accepted();
    test_harness.expect_chain_locked().await;

    // Simulate approximately what would happen on a restart. The session is lost from memory but
    // the chain is still locked.
    test_harness.clear_workspace_sessions();

    // Run the workflow on init
    test_harness.countersigning_tx.trigger(&"init");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // The session state is lost, and we haven't published anything, so the session should be abandoned.
    // We don't get a signal in this case, so we just have to check that the chain gets unlocked.
    test_harness.expect_chain_unlocked().await;

    test_harness.expect_empty_workspace();
    test_harness.expect_no_pending_signals();
}

#[tokio::test(flavor = "multi_thread")]
async fn receive_signatures_and_complete() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(60), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    let signatures = vec![
        bob.produce_signature(&session_data, &entry_hash, test_harness.keystore.clone())
            .await,
        test_harness
            .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
            .await,
    ];

    // Expect to receive a publish event.
    test_harness.reconfigure_network(|mut net| {
        net.expect_publish_countersign()
            .return_once(|_, _, _| Ok(()));
        net
    });

    // Receive the signatures as though they were coming in from a witness.
    countersigning_success(
        test_harness.test_space.space.clone(),
        test_harness.author.clone(),
        signatures,
        test_harness.countersigning_tx.clone(),
    )
    .await;

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // One run should be enough when we got valid signatures and the session is now completed.
    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
}

#[tokio::test(flavor = "multi_thread")]
async fn recover_from_commit_when_other_agent_abandons() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Do nothing for Bob, he's either not committed or not published a signature. For the purpose
    // of this test it really doesn't matter what went wrong on his end.

    // Now for the sake of recovery, let's suppose that we can initially find no activity for Bob.
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Run our workflow, which should trigger itself to spot the timed out session and move it to
    // the unknown state for recovery.
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    // This is where we'll stay unless Bob takes some action
    let resolution = test_harness.expect_session_in_unknown_state();
    assert!(resolution.is_some());
    let resolution = resolution.unwrap();
    assert_eq!(1, resolution.attempts);
    assert_eq!(1, resolution.outcomes.len());
    let bob_resolution = &resolution.outcomes[0];
    assert_eq!(bob.agent, bob_resolution.agent);
    assert_eq!(3, bob_resolution.decisions.len());
    assert!(bob_resolution
        .decisions
        .iter()
        .all(|d| *d == SessionCompletionDecision::Indeterminate));

    // Now let's help the recovery, Bob publishes some other activity
    let activity_response = bob.other_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    // Run the workflow again, this time we should spot the new activity and abandon the session
    test_harness.countersigning_tx.trigger(&"test");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_abandoned_signal().await;

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
}

#[tokio::test(flavor = "multi_thread")]
async fn recover_from_commit_when_other_agent_completes() {
    holochain_trace::test_run();

    let dna_hash = fixt!(DnaHash);
    let mut test_harness = TestHarness::new(dna_hash).await;

    let bob = test_harness.new_remote_agent().await;

    let request = test_preflight_request(&test_harness, std::time::Duration::from_secs(1), &bob);
    let my_acceptance = test_harness
        .accept_countersigning_request(request.clone())
        .await
        .unwrap();

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let bob_acceptance = bob
        .accept_preflight_request(request.clone(), test_harness.keystore.clone())
        .await;

    let (session_data, entry, entry_hash) =
        test_harness.build_session_data(request.clone(), vec![my_acceptance, bob_acceptance]);

    test_harness
        .commit_countersigning_entry(&session_data, entry.clone(), entry_hash.clone())
        .await;

    // Now we've committed out entry, we are waiting for a signature from Bob. For this test, we're
    // not going to receive it. Wait for the session to time out instead.

    // Initially, find no data for Bob
    let activity_response = bob.no_activity_response();
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net
        }
    });

    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    let resolution = test_harness.expect_session_in_unknown_state();
    assert!(resolution.is_some());

    test_harness.expect_session_in_unknown_state();

    // Now Bob's completed session shows up with an AAA
    let activity_response = bob
        .complete_session_activity_response(
            &session_data,
            entry.clone(),
            &entry_hash,
            test_harness.keystore.clone(),
        )
        .await;
    test_harness.reconfigure_network({
        let activity_response = activity_response.clone();
        move |mut net| {
            net.expect_authority_for_hash().returning(|_| Ok(true));

            net.expect_get_agent_activity().returning({
                let activity_response = activity_response.clone();
                move |_, _, _| Ok(vec![activity_response.clone()])
            });

            net.expect_publish_countersign()
                .return_once(|_, _, _| Ok(()));

            net
        }
    });

    test_harness.countersigning_tx.trigger(&"test");
    test_harness
        .respond_to_countersigning_workflow_signal()
        .await;

    test_harness.expect_success_signal().await;
    test_harness.expect_publish_and_integrate();

    test_harness.expect_no_pending_signals();
    test_harness.expect_empty_workspace();
}

struct TestHarness {
    dna_hash: DnaHash,
    test_space: TestSpace,
    network: Arc<MockHolochainP2pDnaT>,
    signal_tx: Sender<Signal>,
    signal_rx: Receiver<Signal>,
    keystore: MetaLairClient,
    author: AgentPubKey,
    countersigning_tx: TriggerSender,
    countersigning_rx: TriggerReceiver,
    integration_tx: TriggerSender,
    integration_rx: TriggerReceiver,
    publish_tx: TriggerSender,
    publish_rx: TriggerReceiver,
    remote_agents: usize,
}

/// Test driver implementation
impl TestHarness {
    async fn new(dna_hash: DnaHash) -> Self {
        let test_space = TestSpace::new(dna_hash.clone());
        let network = MockHolochainP2pDnaT::new();
        let signal = tokio::sync::broadcast::channel::<Signal>(1);
        let keystore = holochain_keystore::test_keystore();
        let author = keystore.new_sign_keypair_random().await.unwrap();
        let countersigning_trigger = TriggerSender::new();
        let integration_trigger = TriggerSender::new();
        let publish_trigger = TriggerSender::new();

        source_chain::genesis(
            test_space
                .space
                .get_or_create_authored_db(author.clone())
                .unwrap(),
            test_space.space.dht_db.clone(),
            &DhtDbQueryCache::new(test_space.space.dht_db.clone().into()),
            keystore.clone(),
            dna_hash.clone(),
            author.clone(),
            None,
            None,
        )
        .await
        .unwrap();

        Self {
            dna_hash,
            test_space,
            network: Arc::new(network),
            signal_tx: signal.0,
            signal_rx: signal.1,
            keystore,
            author,
            countersigning_tx: countersigning_trigger.0,
            countersigning_rx: countersigning_trigger.1,
            integration_tx: integration_trigger.0,
            integration_rx: integration_trigger.1,
            publish_tx: publish_trigger.0,
            publish_rx: publish_trigger.1,
            remote_agents: 0,
        }
    }

    async fn new_remote_agent(&mut self) -> RemoteAgent {
        self.remote_agents += 1;
        RemoteAgent {
            agent: self.keystore.new_sign_keypair_random().await.unwrap(),
            agent_index: self.remote_agents,
            chain_head: ChainHead {
                action_seq: 32,
                hash: fixt!(ActionHash),
            },
        }
    }

    fn reconfigure_network(
        &mut self,
        apply: impl Fn(MockHolochainP2pDnaT) -> MockHolochainP2pDnaT,
    ) {
        let network = apply(MockHolochainP2pDnaT::new());
        self.network = Arc::new(network);
    }

    async fn accept_countersigning_request(
        &self,
        request: PreflightRequest,
    ) -> WorkflowResult<PreflightRequestAcceptance> {
        accept_countersigning_request(
            self.test_space.space.clone(),
            self.keystore.clone(),
            self.author.clone(),
            request,
            self.countersigning_tx.clone(),
        )
        .await
    }

    async fn respond_to_countersigning_workflow_signal(&mut self) {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.countersigning_rx.listen(),
        )
        .await
        .expect("Didn't receive a trigger in time")
        .unwrap();

        let outcome = countersigning_workflow(
            self.test_space.space.clone(),
            self.network.clone(),
            CellId::new(self.dna_hash.clone(), self.author.clone()),
            self.signal_tx.clone(),
            self.countersigning_tx.clone(),
            self.integration_tx.clone(),
            self.publish_tx.clone(),
        )
        .await
        .unwrap();

        assert_eq!(WorkComplete::Complete, outcome);
    }

    async fn unlock_chain(&self) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        authored
            .write_async({
                let author = self.author.clone();
                move |txn| unlock_chain(txn, &author)
            })
            .await
            .unwrap();
    }

    fn clear_workspace_sessions(&self) {
        self.test_space
            .space
            .countersigning_workspace
            .inner
            .share_mut(|w, _| {
                w.sessions.clear();
                Ok(())
            })
            .unwrap();
    }

    fn build_session_data(
        &self,
        request: PreflightRequest,
        acceptances: Vec<PreflightRequestAcceptance>,
    ) -> (CounterSigningSessionData, Entry, EntryHash) {
        let session_data = CounterSigningSessionData::try_new(
            request,
            acceptances
                .into_iter()
                .filter_map(|a| match a {
                    PreflightRequestAcceptance::Accepted(a) => Some((a.agent_state, a.signature)),
                    _ => None,
                })
                .collect(),
            vec![],
        )
        .unwrap();

        let entry = Entry::CounterSign(Box::new(session_data.clone()), fixt!(AppEntryBytes));
        let entry_hash = EntryHash::with_data_sync(&entry);

        (session_data, entry, entry_hash)
    }

    async fn commit_countersigning_entry(
        &self,
        session_data: &CounterSigningSessionData,
        entry: Entry,
        entry_hash: EntryHash,
    ) -> SignedAction {
        let my_action = Action::from_countersigning_data(
            entry_hash.clone(),
            session_data,
            self.author.clone(),
            weigh_placeholder(),
        )
        .unwrap();
        let hashed = ActionHashed::from_content_sync(my_action.clone());
        let sah = SignedActionHashed::sign(&self.keystore, hashed)
            .await
            .unwrap();

        let signed = SignedAction::from(sah.clone());

        let store_entry_op = ChainOp::StoreEntry(
            fixt!(Signature),
            my_action.clone().try_into().unwrap(),
            entry.clone(),
        );
        let dht_op = DhtOp::ChainOp(Box::new(store_entry_op));
        let dht_op = DhtOpHashed::from_content_sync(dht_op);

        self.test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap()
            .write_async(move |txn| -> StateMutationResult<()> {
                insert_action(txn, &sah)?;
                insert_entry(txn, &entry_hash, &entry)?;
                insert_op(txn, &dht_op)?;

                Ok(())
            })
            .await
            .unwrap();

        signed
    }
}

/// Assertion query implementation
impl TestHarness {
    fn expect_empty_workspace(&self) {
        let count = self
            .test_space
            .space
            .countersigning_workspace
            .inner
            .share_ref(|w| Ok(w.sessions.len()))
            .unwrap();

        assert_eq!(0, count);
    }

    fn expect_session_accepted(&self) {
        let maybe_found = self
            .test_space
            .space
            .countersigning_workspace
            .inner
            .share_ref(|w| Ok(w.sessions.get(&self.author).cloned()))
            .unwrap();

        assert!(maybe_found.is_some());

        match maybe_found.unwrap() {
            CountersigningSessionState::Accepted(_) => {}
            _ => panic!("Session not in accepted state"),
        }
    }

    fn expect_session_in_unknown_state(&self) -> Option<SessionResolutionSummary> {
        let maybe_found = self
            .test_space
            .space
            .countersigning_workspace
            .inner
            .share_ref(|w| Ok(w.sessions.get(&self.author).cloned()))
            .unwrap();

        assert!(maybe_found.is_some());

        match maybe_found {
            Some(CountersigningSessionState::Unknown { resolution, .. }) => resolution,
            state => panic!("Session not in unknown state: {:?}", state),
        }
    }

    async fn expect_chain_locked(&self) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        let lock = authored
            .read_async({
                let author = self.author.clone();
                move |txn| get_chain_lock(&txn, &author)
            })
            .await
            .unwrap();

        assert!(lock.is_some());
    }

    pub async fn expect_chain_unlocked(&self) {
        let authored = self
            .test_space
            .space
            .get_or_create_authored_db(self.author.clone())
            .unwrap();
        let lock = authored
            .read_async({
                let author = self.author.clone();
                move |txn| get_chain_lock(&txn, &author)
            })
            .await
            .unwrap();

        assert!(lock.is_none());
    }

    pub async fn expect_abandoned_signal(&mut self) {
        let signal = tokio::time::timeout(std::time::Duration::from_secs(1), self.signal_rx.recv())
            .await
            .expect("Didn't receive a signal in time")
            .unwrap();

        assert_matches!(
            signal,
            Signal::System(SystemSignal::AbandonedCountersigning(_))
        );
    }

    pub async fn expect_success_signal(&mut self) {
        let signal = tokio::time::timeout(std::time::Duration::from_secs(1), self.signal_rx.recv())
            .await
            .expect("Didn't receive a signal in time")
            .unwrap();

        assert_matches!(
            signal,
            Signal::System(SystemSignal::SuccessfulCountersigning(_))
        );
    }

    pub fn expect_publish_and_integrate(&mut self) {
        self.integration_rx.try_recv().unwrap();
        self.publish_rx.try_recv().unwrap();
    }

    fn expect_no_pending_signals(&mut self) {
        let signal = self.signal_rx.try_recv().ok();
        assert!(signal.is_none());

        let trigger = self.countersigning_rx.try_recv();
        assert!(trigger.is_none());

        let trigger = self.integration_rx.try_recv();
        assert!(trigger.is_none());

        let trigger = self.publish_rx.try_recv();
        assert!(trigger.is_none());
    }
}

struct RemoteAgent {
    agent: AgentPubKey,
    agent_index: usize,
    chain_head: ChainHead,
}

impl RemoteAgent {
    async fn accept_preflight_request(
        &self,
        request: PreflightRequest,
        keystore: MetaLairClient,
    ) -> PreflightRequestAcceptance {
        let agent_state = CounterSigningAgentState::new(
            self.agent_index as u8,
            self.chain_head.hash.clone(),
            self.chain_head.action_seq,
        );
        let response_data =
            PreflightResponse::encode_fields_for_signature(&request, &agent_state).unwrap();
        let signature = keystore
            .sign(self.agent.clone(), response_data.into())
            .await
            .unwrap();

        PreflightRequestAcceptance::Accepted(
            PreflightResponse::try_new(request.clone(), agent_state, signature).unwrap(),
        )
    }

    async fn produce_signature(
        &self,
        session_data: &CounterSigningSessionData,
        entry_hash: &EntryHash,
        keystore: MetaLairClient,
    ) -> SignedAction {
        let action = Action::from_countersigning_data(
            entry_hash.clone(),
            session_data,
            self.agent.clone(),
            weigh_placeholder(),
        )
        .unwrap();

        let hashed = ActionHashed::from_content_sync(action.clone());
        let sah = SignedActionHashed::sign(&keystore, hashed).await.unwrap();

        SignedAction::from(sah)
    }

    fn no_activity_response(&self) -> AgentActivityResponse {
        AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(vec![]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Valid(self.chain_head.clone()),
            highest_observed: None,
            warrants: vec![],
        }
    }

    fn other_activity_response(&self) -> AgentActivityResponse {
        let action = Action::Create(fixt!(Create));

        AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(vec![Record::new(
                SignedActionHashed::new_unchecked(action, fixt!(Signature)),
                Some(fixt!(Entry)),
            )]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Valid(ChainHead {
                action_seq: self.chain_head.action_seq + 1,
                hash: fixt!(ActionHash), // Won't match the action activity hash, doesn't matter
            }),
            highest_observed: None,
            warrants: vec![],
        }
    }

    async fn complete_session_activity_response(
        &self,
        session_data: &CounterSigningSessionData,
        entry: Entry,
        entry_hash: &EntryHash,
        keystore: MetaLairClient,
    ) -> AgentActivityResponse {
        let signed_action = self
            .produce_signature(session_data, entry_hash, keystore)
            .await;
        let signature = signed_action.signature().clone();

        AgentActivityResponse {
            agent: self.agent.clone(),
            valid_activity: ChainItems::Full(vec![Record::new(
                SignedActionHashed::with_presigned(
                    ActionHashed::from_content_sync(signed_action.into_data()),
                    signature,
                ),
                Some(entry),
            )]),
            rejected_activity: ChainItems::NotRequested,
            status: ChainStatus::Valid(ChainHead {
                action_seq: self.chain_head.action_seq + 1,
                hash: fixt!(ActionHash), // Won't match the action activity hash, doesn't matter
            }),
            highest_observed: None,
            warrants: vec![],
        }
    }
}

fn test_preflight_request(
    test_harness: &TestHarness,
    duration: std::time::Duration,
    other: &RemoteAgent,
) -> PreflightRequest {
    PreflightRequest::try_new(
        fixt!(EntryHash),
        vec![
            (test_harness.author.clone(), vec![]),
            (other.agent.clone(), vec![]),
        ],
        vec![],
        0,
        false,
        CounterSigningSessionTimes {
            start: Timestamp::now(),
            end: Timestamp::now().add(duration).unwrap(),
        },
        ActionBase::Create(CreateBase::new(fixt!(EntryType))),
        PreflightBytes(vec![]),
    )
    .unwrap()
}
