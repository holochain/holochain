use super::*;

pub(crate) async fn spawn_test_agent(
    harness_chan: HarnessEventChannel,
    config: KitsuneP2pConfig,
) -> Result<(Arc<KitsuneAgent>, ghost_actor::GhostSender<KitsuneP2p>), KitsuneP2pError> {
    let agent: Arc<KitsuneAgent> = TestVal::test_val();
    let (p2p, evt) = spawn_kitsune_p2p(config).await?;

    let builder = ghost_actor::actor_builder::GhostActorBuilder::new();

    let channel_factory = builder.channel_factory().clone();

    channel_factory.attach_receiver(evt).await?;

    tokio::task::spawn(builder.spawn(AgentHarness::new(harness_chan)));

    Ok((agent, p2p))
}

struct AgentHarness {
    harness_chan: HarnessEventChannel,
    agent_store: HashMap<Arc<KitsuneAgent>, Arc<AgentInfoSigned>>,
}

impl AgentHarness {
    pub fn new(harness_chan: HarnessEventChannel) -> Self {
        Self {
            harness_chan,
            agent_store: HashMap::new(),
        }
    }
}

impl ghost_actor::GhostControlHandler for AgentHarness {}

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

    fn handle_call(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        _payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<Vec<u8>> {
        unimplemented!()
    }

    fn handle_notify(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        _payload: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        unimplemented!()
    }

    fn handle_gossip(
        &mut self,
        _space: Arc<super::KitsuneSpace>,
        _to_agent: Arc<super::KitsuneAgent>,
        _from_agent: Arc<super::KitsuneAgent>,
        _op_hash: Arc<super::KitsuneOpHash>,
        _op_data: Vec<u8>,
    ) -> KitsuneP2pEventHandlerResult<()> {
        unimplemented!()
    }

    fn handle_fetch_op_hashes_for_constraints(
        &mut self,
        _input: FetchOpHashesForConstraintsEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<Arc<super::KitsuneOpHash>>> {
        unimplemented!()
    }

    fn handle_fetch_op_hash_data(
        &mut self,
        _input: FetchOpHashDataEvt,
    ) -> KitsuneP2pEventHandlerResult<Vec<(Arc<super::KitsuneOpHash>, Vec<u8>)>> {
        unimplemented!()
    }

    fn handle_sign_network_data(
        &mut self,
        _input: SignNetworkDataEvt,
    ) -> KitsuneP2pEventHandlerResult<KitsuneSignature> {
        unimplemented!()
    }
}
