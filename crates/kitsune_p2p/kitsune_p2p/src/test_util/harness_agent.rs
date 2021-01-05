use super::*;

ghost_actor::ghost_chan! {
    /// controller for test harness agent actor
    pub(crate) chan HarnessAgentControl<KitsuneP2pError> {
        /// dump agent info from peer_store
        fn dump_agent_info() -> Vec<Arc<AgentInfoSigned>>;

        /// inject a bunch of agent info
        fn inject_agent_info(info: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>) -> ();

        /// inject data to be gradually gossiped
        fn inject_gossip_data(data: String) -> Arc<KitsuneOpHash>;

        /// dump all local gossip data from this agent
        fn dump_local_gossip_data() -> HashMap<Arc<KitsuneOpHash>, String>;

        /// dump all local peer data from this agent
        fn dump_local_peer_data() -> HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>;
    }
}

pub(crate) async fn spawn_test_agent(
    harness_chan: HarnessEventChannel,
    config: KitsuneP2pConfig,
) -> Result<
    (
        Arc<KitsuneAgent>,
        ghost_actor::GhostSender<KitsuneP2p>,
        ghost_actor::GhostSender<HarnessAgentControl>,
    ),
    KitsuneP2pError,
> {
    let (p2p, evt) = spawn_kitsune_p2p(
        config,
        kitsune_p2p_proxy::TlsConfig::new_ephemeral().await.unwrap(),
    )
    .await?;

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    channel_factory.attach_receiver(evt).await?;

    let control = channel_factory
        .create_channel::<HarnessAgentControl>()
        .await?;

    let harness = AgentHarness::new(harness_chan).await?;
    let agent = harness.agent.clone();
    tokio::task::spawn(builder.spawn(harness));

    Ok((agent, p2p, control))
}

use lair_keystore_api::entry::EntrySignEd25519;
use lair_keystore_api::internal::sign_ed25519::*;

struct AgentHarness {
    agent: Arc<KitsuneAgent>,
    priv_key: SignEd25519PrivKey,
    harness_chan: HarnessEventChannel,
    agent_store: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>,
    gossip_store: HashMap<Arc<KitsuneOpHash>, String>,
}

impl AgentHarness {
    pub async fn new(harness_chan: HarnessEventChannel) -> Result<Self, KitsuneP2pError> {
        let EntrySignEd25519 { priv_key, pub_key } = sign_ed25519_keypair_new_from_entropy()
            .await
            .map_err(KitsuneP2pError::other)?;
        let pub_key = (**pub_key).clone();
        let agent: Arc<KitsuneAgent> = Arc::new(KitsuneAgent::new(pub_key));
        Ok(Self {
            agent,
            priv_key,
            harness_chan,
            agent_store: HashMap::new(),
            gossip_store: HashMap::new(),
        })
    }
}

impl ghost_actor::GhostControlHandler for AgentHarness {}

impl ghost_actor::GhostHandler<HarnessAgentControl> for AgentHarness {}

impl HarnessAgentControlHandler for AgentHarness {
    fn handle_dump_agent_info(
        &mut self,
    ) -> HarnessAgentControlHandlerResult<Vec<Arc<AgentInfoSigned>>> {
        let all = self.agent_store.values().map(|a| a.clone()).collect();
        Ok(async move { Ok(all) }.boxed().into())
    }

    fn handle_inject_agent_info(
        &mut self,
        info: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>,
    ) -> HarnessAgentControlHandlerResult<()> {
        self.agent_store.extend(info);
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_inject_gossip_data(
        &mut self,
        data: String,
    ) -> HarnessAgentControlHandlerResult<Arc<KitsuneOpHash>> {
        let op_hash: Arc<KitsuneOpHash> = TestVal::test_val();
        self.gossip_store.insert(op_hash.clone(), data);
        Ok(async move { Ok(op_hash) }.boxed().into())
    }

    fn handle_dump_local_gossip_data(
        &mut self,
    ) -> HarnessAgentControlHandlerResult<HashMap<Arc<KitsuneOpHash>, String>> {
        let out = self.gossip_store.clone();
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_dump_local_peer_data(
        &mut self,
    ) -> HarnessAgentControlHandlerResult<HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>> {
        let out = self.agent_store.clone();
        Ok(async move { Ok(out) }.boxed().into())
    }
}

impl ghost_actor::GhostHandler<KitsuneP2pEvent> for AgentHarness {}

impl KitsuneP2pEventHandler for AgentHarness {
    fn handle_put_agent_info_signed(
        &mut self,
        input: PutAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<()> {
        let info = Arc::new(input.agent_info_signed);
        self.agent_store.insert(input.agent.clone(), info.clone());
        self.harness_chan.publish(HarnessEventType::StoreAgentInfo {
            agent: (&input.agent).into(),
            agent_info: info,
        });
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_get_agent_info_signed(
        &mut self,
        input: GetAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Option<crate::types::agent_store::AgentInfoSigned>> {
        let res = self.agent_store.get(&input.agent).map(|i| (**i).clone());
        Ok(async move { Ok(res) }.boxed().into())
    }

    fn handle_query_agent_info_signed(
        &mut self,
        _input: QueryAgentInfoSignedEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<crate::types::agent_store::AgentInfoSigned>> {
        let out = self.agent_store.values().map(|a| (**a).clone()).collect();
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_call(
        &mut self,
        space: Arc<super::KitsuneSpace>,
        to_agent: Arc<super::KitsuneAgent>,
        from_agent: Arc<super::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        let data = String::from_utf8_lossy(&payload);
        self.harness_chan.publish(HarnessEventType::Call {
            space: space.into(),
            to_agent: to_agent.into(),
            from_agent: from_agent.into(),
            payload: data.to_string(),
        });
        let data = format!("echo: {}", data);
        let data = data.into_bytes();
        Ok(async move { Ok(data) }.boxed().into())
    }

    fn handle_notify(
        &mut self,
        space: Arc<super::KitsuneSpace>,
        to_agent: Arc<super::KitsuneAgent>,
        from_agent: Arc<super::KitsuneAgent>,
        payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        let data = String::from_utf8_lossy(&payload);
        self.harness_chan.publish(HarnessEventType::Notify {
            space: space.into(),
            to_agent: to_agent.into(),
            from_agent: from_agent.into(),
            payload: data.to_string(),
        });
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_gossip(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        op_hash: Arc<super::KitsuneOpHash>,
        op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        let op_data = String::from_utf8_lossy(&op_data).to_string();
        self.harness_chan.publish(HarnessEventType::Gossip {
            op_hash: (&op_hash).into(),
            op_data: op_data.clone(),
        });
        self.gossip_store.insert(op_hash, op_data);
        Ok(async move { Ok(()) }.boxed().into())
    }

    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        _input: FetchOpHashesForConstraintsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<Arc<super::KitsuneOpHash>>> {
        let hashes: Vec<Arc<super::KitsuneOpHash>> = self.gossip_store.keys().cloned().collect();
        let slug_hashes: Vec<Slug> = hashes.iter().map(|h| h.into()).collect();
        tracing::trace!(?slug_hashes, "FETCH_OP_HASHES");
        Ok(async move { Ok(hashes) }.boxed().into())
    }

    fn handle_fetch_op_hash_data(
        &mut self,
        input: FetchOpHashDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<super::KitsuneOpHash>, Vec<u8>)>> {
        let mut out = Vec::new();
        for hash in input.op_hashes {
            if let Some(op) = self.gossip_store.get(&hash) {
                out.push((hash.clone(), op.clone().into_bytes()));
            }
        }
        Ok(async move { Ok(out) }.boxed().into())
    }

    fn handle_sign_network_data(
        &mut self,
        input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        let sig = sign_ed25519(self.priv_key.clone(), input.data);
        Ok(async move {
            let sig = sig.await.map_err(KitsuneP2pError::other)?;
            let sig: Vec<u8> = (**sig).clone();
            Ok(sig.into())
        }
        .boxed()
        .into())
    }
}
