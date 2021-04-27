use super::*;

pub struct SparseDataMap {
    space: Arc<KitsuneSpace>,
    evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    map: HashMap<Arc<MetaOpKey>, Arc<MetaOpData>>,
}

impl SparseDataMap {
    pub fn new(
        space: Arc<KitsuneSpace>,
        evt_sender: futures::channel::mpsc::Sender<event::KitsuneP2pEvent>,
    ) -> Self {
        Self {
            space,
            evt_sender,
            map: HashMap::new(),
        }
    }

    pub fn inject_agent_info(
        &mut self,
        agent_info: AgentInfoSigned,
    ) -> Arc<MetaOpKey> {
        let key = Arc::new(MetaOpKey::Agent(Arc::new(agent_info.as_agent_ref().clone())));
        let data = Arc::new(MetaOpData::Agent(agent_info));
        self.inject_meta(key.clone(), data);
        key
    }

    pub fn inject_meta(
        &mut self,
        key: Arc<MetaOpKey>,
        data: Arc<MetaOpData>,
    ) {
        self.map.insert(key, data);
    }

    pub async fn get(
        &mut self,
        agent: &Arc<KitsuneAgent>,
        key: &Arc<MetaOpKey>,
    ) -> KitsuneResult<Arc<MetaOpData>> {
        use crate::event::*;
        if let Some(data) = self.map.get(key) {
            return Ok(data.clone());
        }
        match &**key {
            MetaOpKey::Op(key) => {
                let mut op = self.evt_sender.fetch_op_hash_data(FetchOpHashDataEvt {
                    space: self.space.clone(),
                    agent: agent.clone(),
                    op_hashes: vec![key.clone()],
                }).await.map_err(KitsuneError::other)?;

                if op.len() != 1 {
                    return Err("invalid results".into());
                }

                let (key, data) = op.remove(0);
                let data = Arc::new(MetaOpData::Op(key.clone(), data));
                let key = Arc::new(MetaOpKey::Op(key));

                self.map.insert(key.clone(), data.clone());
                Ok(data)
            }
            MetaOpKey::Agent(_) => unreachable!(),
        }
    }
}
