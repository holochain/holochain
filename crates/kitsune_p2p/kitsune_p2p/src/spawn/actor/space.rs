use super::*;

/// Normally network lookups / connections will be async / take some time.
/// While we are in "short-circuit-only" mode - we just need to allow some
/// time for other agenst to be connected to this conductor.
/// This value does NOT have to be correct, it just has to work.
const NET_CONNECT_INTERVAL_MS: u64 = 20;

/// Max amount of time we should wait for connections to be established.
const NET_CONNECT_MAX_MS: u64 = 2000;

/// Local helper struct for associating info with a connected agent.
struct AgentInfo {
    #[allow(dead_code)]
    agent: Arc<KitsuneAgent>,
}

/// A Kitsune P2p Node can track multiple "spaces" -- Non-interacting namespaced
/// areas that share common transport infrastructure for communication.
pub(crate) struct Space {
    space: Arc<KitsuneSpace>,
    internal_sender: KitsuneP2pInternalSender<Internal>,
    evt_sender: futures::channel::mpsc::Sender<KitsuneP2pEvent>,
    agents: HashMap<Arc<KitsuneAgent>, AgentInfo>,
}

impl Space {
    /// space constructor
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

    /// how many agents are connected to this space
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    pub fn list_agents(&self) -> Vec<Arc<KitsuneAgent>> {
        self.agents.keys().cloned().collect()
    }

    /// process an incoming join request for an agent -- add them to the space
    pub fn handle_join(&mut self, agent: Arc<KitsuneAgent>) -> KitsuneP2pHandlerResult<()> {
        match self.agents.entry(agent.clone()) {
            Entry::Occupied(_) => (),
            Entry::Vacant(entry) => {
                entry.insert(AgentInfo { agent });
            }
        }
        Ok(async move { Ok(()) }.boxed().into())
    }

    /// process an incoming leave request for an agent -- remove them from the space
    pub fn handle_leave(&mut self, agent: Arc<KitsuneAgent>) -> KitsuneP2pHandlerResult<()> {
        self.agents.remove(&agent);
        Ok(async move { Ok(()) }.boxed().into())
    }

    /// process an "immediate" request
    /// that is - attempt to send a request and return an error on failure
    /// this helper doesn't do any waiting / retrying.
    pub fn handle_internal_immediate_request(
        &mut self,
        agent: Arc<KitsuneAgent>,
        data: Arc<Vec<u8>>,
    ) -> KitsuneP2pHandlerResult<Vec<u8>> {
        // Right now we are only implementing the "short-circuit"
        // that routes messages to other agents joined on this same system.
        // I.e. we don't bother with peer discovery because we know the
        // remote is local.
        if !self.agents.contains_key(&agent) {
            return Err(KitsuneP2pError::RoutingAgentError(agent));
        }

        // that agent *is* joined - let's forward the request
        let space = self.space.clone();

        // clone the event sender
        let mut evt_sender = self.evt_sender.clone();

        // As this is a short-circuit - we need to decode the data inline - here.
        // In the future, we will probably need to branch here, so the real
        // networking can forward the encoded data. Or, split immediate_request
        // into two variants, one for short-circuit, and one for real networking.
        let data = wire::Wire::decode((*data).clone())?;

        match data {
            wire::Wire::Request(data) => {
                Ok(async move { evt_sender.request(space, agent, data).await }
                    .boxed()
                    .into())
            }
            wire::Wire::Broadcast(data) => {
                Ok(async move {
                    evt_sender.broadcast(space, agent, data).await?;
                    // broadcast doesn't return anything...
                    Ok(vec![])
                }
                .boxed()
                .into())
            }
        }
    }

    /// send / process a request - waiting / retrying as appropriate
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
