use super::*;

/// Normally network lookups / connections will be async / take some time.
/// While we are in "short-circuit-only" mode - we just need to allow some
/// time for other agenst to be connected to this conductor.
/// This value does NOT have to be correct, it just has to work.
const NET_CONNECT_INTERVAL_MS: u64 = 20;

/// Max amount of time we should wait for connections to be established.
const NET_CONNECT_MAX_MS: u64 = 2000;

struct AgentInfo {
    #[allow(dead_code)]
    agent: Arc<KitsuneAgent>,
}

pub(crate) struct Space {
    space: Arc<KitsuneSpace>,
    internal_sender: KitsuneP2pInternalSender<Internal>,
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

    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn handle_join(&mut self, agent: Arc<KitsuneAgent>) -> KitsuneP2pHandlerResult<()> {
        match self.agents.entry(agent.clone()) {
            Entry::Occupied(_) => (),
            Entry::Vacant(entry) => {
                entry.insert(AgentInfo { agent });
            }
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    pub fn handle_leave(&mut self, agent: Arc<KitsuneAgent>) -> KitsuneP2pHandlerResult<()> {
        self.agents.remove(&agent);
        Ok(async move { Ok(()) }.boxed().into())
    }

    pub fn handle_internal_immediate_request(
        &mut self,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        // right now we are only implementing the "short-circuit"
        // that routes messages to other agents joined on this same system.
        if !self.agents.contains_key(&agent) {
            return Err(KitsuneP2pError::RoutingAgentError(agent));
        }

        // that agent *is* joined - let's forward the request
        let space = self.space.clone();

        // clone the event sender
        let mut evt_sender = self.evt_sender.clone();

        Ok(async move { evt_sender.request(space, agent, data).await }
            .boxed()
            .into())
    }

    pub fn handle_request(
        &mut self,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        let space = self.space.clone();
        let mut internal_sender = self.internal_sender.clone();
        Ok(async move {
            let start = std::time::Instant::now();

            loop {
                // attempt to send the request right now
                let err = match internal_sender
                    .ghost_actor_internal()
                    .immediate_request(space.clone(), agent.clone(), data.clone())
                    .await
                {
                    Ok(res) => return Ok(res),
                    Err(e) => Err(e),
                };

                // the attempt failed
                // see if we have been trying too long
                if start.elapsed().as_millis() as u64 > NET_CONNECT_MAX_MS {
                    return err;
                }

                // the attempt failed - wait a bit to allow agents to connect
                tokio::time::delay_for(std::time::Duration::from_millis(NET_CONNECT_INTERVAL_MS))
                    .await;
            }
        }
        .boxed()
        .into())
    }
}
