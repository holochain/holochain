use super::*;

struct AgentInfo {
    #[allow(dead_code)]
    agent_hash: Arc<KitsuneAgent>,
}

pub(crate) struct Space {
    #[allow(dead_code)]
    space: Arc<KitsuneSpace>,
    #[allow(dead_code)]
    internal_sender: KitsuneP2pInternalSender<Internal>,
    #[allow(dead_code)]
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    agents: HashMap<Arc<KitsuneAgent>, AgentInfo>,
}

impl Space {
    pub fn new(
        space: Arc<KitsuneSpace>,
        internal_sender: KitsuneP2pInternalSender<Internal>,
        evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    ) -> Self {
        Self {
            space,
            internal_sender,
            evt_sender,
            agents: HashMap::new(),
        }
    }

    pub fn handle_join(&mut self, agent_hash: Arc<KitsuneAgent>) -> KitsuneP2pHandlerResult<()> {
        match self.agents.entry(agent_hash.clone()) {
            Entry::Occupied(_) => (),
            Entry::Vacant(entry) => {
                entry.insert(AgentInfo { agent_hash });
            }
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    pub fn handle_leave(&mut self, agent_hash: Arc<KitsuneAgent>) -> KitsuneP2pHandlerResult<()> {
        self.agents.remove(&agent_hash);
        Ok(async move { Ok(()) }.boxed().into())
    }

    pub fn handle_request(
        &mut self,
        agent_hash: Arc<KitsuneAgent>,
        data: Vec<u8>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        // right now we are only implementing the "short-circuit"
        // that routes messages to other agents joined on this same system.
        if !self.agents.contains_key(&agent_hash) {
            return Err(KitsuneP2pError::RoutingFailure(format!(
                "agent '{:?}' not joined",
                agent_hash
            )));
        }

        // that agent *is* joined - let's forward the request
        let req = RequestEvt {
            space: (*self.space).clone(),
            agent: (*agent_hash).clone(),
            request: data,
        };

        // clone the event sender
        let mut evt_sender = self.evt_sender.clone();

        Ok(async move { evt_sender.request(req).await }.boxed().into())
    }
}
