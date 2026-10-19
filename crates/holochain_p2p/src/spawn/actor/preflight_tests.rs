use super::*;
use crate::wire::WirePreflightMessage;
use kitsune2_api::{AgentInfo, DhtArc, Timestamp};
use std::sync::mpsc;

const LIVE_UNTIL: i64 = i64::MAX;
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

struct PreflightHarness {
    actor: Arc<HolochainP2pActor>,
    builder: Arc<Builder>,
}

impl PreflightHarness {
    async fn new(orig: DynBootstrapFactory) -> Self {
        let actor = HolochainP2pActor::create(
            HolochainP2pConfig::default(),
            holochain_keystore::test_keystore(),
        )
        .await
        .unwrap();
        let actor = Arc::downcast::<HolochainP2pActor>(actor).unwrap();
        let builder = Builder {
            bootstrap: Arc::new(BootWrapFact {
                compat: actor.compat.clone(),
                preflight: actor.preflight.clone(),
                orig,
            }),
            ..kitsune2_core::default_test_builder()
        }
        .with_default_config()
        .unwrap();
        builder.validate_config().unwrap();
        Self {
            actor,
            builder: Arc::new(builder),
        }
    }

    async fn bootstrap(&self, space: &SpaceId) -> DynBootstrap {
        let blocks = self
            .builder
            .blocks
            .create(self.builder.clone(), space.clone())
            .await
            .unwrap();
        let known_peers = self
            .builder
            .known_peers
            .create(self.builder.clone(), space.clone())
            .await
            .unwrap();
        let peer_store = self
            .builder
            .peer_store
            .create(self.builder.clone(), space.clone(), blocks, known_peers)
            .await
            .unwrap();
        self.builder
            .bootstrap
            .create(self.builder.clone(), peer_store, space.clone())
            .await
            .unwrap()
    }

    async fn outgoing_infos(&self) -> Vec<Arc<AgentInfoSigned>> {
        let bytes = self
            .actor
            .preflight_gather_outgoing(Url::from_str("ws://peer:80/1").unwrap())
            .await
            .unwrap();
        let message = WirePreflightMessage::decode(&bytes).unwrap();
        let mut agents: Vec<_> = message
            .agents
            .iter()
            .map(|encoded| {
                AgentInfoSigned::decode(&kitsune2_core::Ed25519Verifier, encoded.as_bytes())
                    .unwrap()
            })
            .collect();
        agents.sort_by(|a, b| {
            (&a.space, &a.agent, a.created_at).cmp(&(&b.space, &b.agent, b.created_at))
        });
        agents
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn two_spaces_contribute_to_one_preflight() {
    let harness = PreflightHarness::new(Arc::new(crate::test::NoopBootstrapFactory)).await;
    let space_a = DnaHash::from_raw_32(vec![1; 32]).to_k2_space();
    let space_b = DnaHash::from_raw_32(vec![2; 32]).to_k2_space();
    let bootstrap_a = harness.bootstrap(&space_a).await;
    let keystore = &harness.actor.lair_client;
    let alice = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let bob = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let info_a = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_a.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let info_b = AgentInfoSigned::sign(
        &bob,
        AgentInfo {
            agent: bob.agent().clone(),
            space: space_b.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();

    bootstrap_a.put(info_a.clone());
    // Creating another wrapper must not reset the first space's contribution.
    let bootstrap_b = harness.bootstrap(&space_b).await;
    bootstrap_b.put(info_b.clone());

    // The preflight must advertise both spaces, not just the last publisher.
    assert_eq!(harness.outgoing_infos().await, vec![info_a, info_b]);
}

#[tokio::test(flavor = "multi_thread")]
async fn refreshing_one_space_replaces_its_info_without_removing_another_space() {
    let harness = PreflightHarness::new(Arc::new(crate::test::NoopBootstrapFactory)).await;
    let space_a = DnaHash::from_raw_32(vec![1; 32]).to_k2_space();
    let space_b = DnaHash::from_raw_32(vec![2; 32]).to_k2_space();
    let bootstrap_a = harness.bootstrap(&space_a).await;
    let bootstrap_b = harness.bootstrap(&space_b).await;
    let keystore = &harness.actor.lair_client;
    let alice = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let bob = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let original_a = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_a.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let refreshed_a = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_a.clone(),
            created_at: Timestamp::from_micros(2),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let info_b = AgentInfoSigned::sign(
        &bob,
        AgentInfo {
            agent: bob.agent().clone(),
            space: space_b.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();

    // Publish both spaces, then refresh only A.
    bootstrap_a.put(original_a);
    bootstrap_b.put(info_b.clone());
    bootstrap_a.put(refreshed_a.clone());

    // Replace A's old info while retaining B's unchanged info.
    assert_eq!(harness.outgoing_infos().await, vec![refreshed_a, info_b]);
}

#[tokio::test(flavor = "multi_thread")]
async fn same_agent_in_different_spaces_keeps_both_infos() {
    let harness = PreflightHarness::new(Arc::new(crate::test::NoopBootstrapFactory)).await;
    let space_a = DnaHash::from_raw_32(vec![1; 32]).to_k2_space();
    let space_b = DnaHash::from_raw_32(vec![2; 32]).to_k2_space();
    let bootstrap_a = harness.bootstrap(&space_a).await;
    let bootstrap_b = harness.bootstrap(&space_b).await;
    let keystore = &harness.actor.lair_client;
    let alice = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let info_a = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_a.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let info_b = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_b.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();

    // Publish the same signing key in two spaces.
    bootstrap_a.put(info_a.clone());
    bootstrap_b.put(info_b.clone());

    // Infos are distinct by (space, agent), not agent alone.
    assert_eq!(harness.outgoing_infos().await, vec![info_a, info_b]);
}

#[tokio::test(flavor = "multi_thread")]
async fn publishing_another_space_prunes_expired_infos() {
    let harness = PreflightHarness::new(Arc::new(crate::test::NoopBootstrapFactory)).await;
    let space_a = DnaHash::from_raw_32(vec![1; 32]).to_k2_space();
    let space_b = DnaHash::from_raw_32(vec![2; 32]).to_k2_space();
    let bootstrap_a = harness.bootstrap(&space_a).await;
    let bootstrap_b = harness.bootstrap(&space_b).await;
    let keystore = &harness.actor.lair_client;
    let alice = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let bob = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let expired_a = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_a.clone(),
            created_at: Timestamp::from_micros(0),
            expires_at: Timestamp::from_micros(1),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let info_b = AgentInfoSigned::sign(
        &bob,
        AgentInfo {
            agent: bob.agent().clone(),
            space: space_b.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();

    // Seed an expired A info without waiting for wall-clock time.
    bootstrap_a.put(expired_a);
    // Publishing B must also prune expired infos from A.
    bootstrap_b.put(info_b.clone());

    // Only B's live info should remain.
    assert_eq!(harness.outgoing_infos().await, vec![info_b]);
}

#[derive(Debug, Clone)]
struct GatedBootstrap {
    entered: mpsc::Sender<()>,
    release: Arc<Mutex<mpsc::Receiver<()>>>,
}

impl Bootstrap for GatedBootstrap {
    fn put(&self, _info: Arc<AgentInfoSigned>) {
        self.entered.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(WAIT_TIMEOUT)
            .expect("publication was not released");
    }
}

impl BootstrapFactory for GatedBootstrap {
    fn default_config(&self, _config: &mut Config) -> K2Result<()> {
        Ok(())
    }

    fn validate_config(&self, _config: &Config) -> K2Result<()> {
        Ok(())
    }

    fn create(
        &self,
        _builder: Arc<Builder>,
        _peer_store: DynPeerStore,
        _space: SpaceId,
    ) -> BoxFut<'static, K2Result<DynBootstrap>> {
        let bootstrap: DynBootstrap = Arc::new(self.clone());
        Box::pin(async move { Ok(bootstrap) })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_publications_keep_both_spaces_in_preflight() {
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let harness = PreflightHarness::new(Arc::new(GatedBootstrap {
        entered: entered_tx,
        release: Arc::new(Mutex::new(release_rx)),
    }))
    .await;
    let space_a = DnaHash::from_raw_32(vec![1; 32]).to_k2_space();
    let space_b = DnaHash::from_raw_32(vec![2; 32]).to_k2_space();
    let bootstrap_a = harness.bootstrap(&space_a).await;
    let bootstrap_b = harness.bootstrap(&space_b).await;
    let keystore = &harness.actor.lair_client;
    let alice = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let bob = HolochainP2pLocalAgent::new(
        keystore.new_sign_keypair_random().await.unwrap(),
        DhtArc::FULL,
        1,
        keystore.clone(),
    );
    let info_a = AgentInfoSigned::sign(
        &alice,
        AgentInfo {
            agent: alice.agent().clone(),
            space: space_a.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let info_b = AgentInfoSigned::sign(
        &bob,
        AgentInfo {
            agent: bob.agent().clone(),
            space: space_b.clone(),
            created_at: Timestamp::from_micros(1),
            expires_at: Timestamp::from_micros(LIVE_UNTIL),
            is_tombstone: false,
            url: Some(Url::from_str("ws://local:80/1").unwrap()),
            storage_arc: DhtArc::FULL,
        },
    )
    .await
    .unwrap();
    let start = Arc::new(std::sync::Barrier::new(3));

    let publish_a = {
        let start = start.clone();
        let info = info_a.clone();
        std::thread::spawn(move || {
            start.wait();
            bootstrap_a.put(info);
        })
    };
    let publish_b = {
        let start = start.clone();
        let info = info_b.clone();
        std::thread::spawn(move || {
            start.wait();
            bootstrap_b.put(info);
        })
    };
    // Start both put calls together.
    start.wait();

    // Both calls have published their preflight but cannot return until released.
    let first_entered = entered_rx.recv_timeout(WAIT_TIMEOUT);
    let second_entered = entered_rx.recv_timeout(WAIT_TIMEOUT);
    let outgoing = harness.outgoing_infos().await;
    // Release and join both publishers before asserting.
    let _ = release_tx.send(());
    let _ = release_tx.send(());
    let result_a = publish_a.join();
    let result_b = publish_b.join();
    first_entered.expect("first publication did not reach bootstrap");
    second_entered.expect("second publication did not reach bootstrap");
    result_a.unwrap();
    result_b.unwrap();

    // Neither concurrent publication may erase the other space.
    assert_eq!(outgoing, vec![info_a, info_b]);
}
