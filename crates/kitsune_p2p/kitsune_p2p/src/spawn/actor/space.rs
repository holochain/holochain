use super::*;

struct AgentInfo {
    #[allow(dead_code)]
    agent_hash: Arc<KitsuneAgent>,
}

pub(crate) struct Space {
    #[allow(dead_code)]
    internal_sender: KitsuneP2pInternalSender<Internal>,
    agents: HashMap<Arc<KitsuneAgent>, AgentInfo>,
}

impl Space {
    pub fn new(internal_sender: KitsuneP2pInternalSender<Internal>) -> Self {
        Self {
            internal_sender,
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
}
