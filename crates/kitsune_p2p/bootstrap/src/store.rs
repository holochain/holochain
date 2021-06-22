use std::{collections::HashMap, sync::Arc};

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneSpace},
    bootstrap::RandomQuery,
    codec::rmp_encode,
};
use parking_lot::RwLock;
use rand::seq::IteratorRandom;

#[derive(Clone, Debug)]
pub(crate) struct Store(Arc<RwLock<HashMap<KitsuneSpace, HashMap<KitsuneAgent, AgentInfoSigned>>>>);

impl Store {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn put(&self, info: AgentInfoSigned) {
        let mut lock = self.0.write();
        let space_map = lock
            .entry((*info.space).clone())
            .or_insert_with(HashMap::new);
        space_map.insert((*info.agent).clone(), info);
    }

    pub fn random(&self, query: RandomQuery) -> Vec<Vec<u8>> {
        // TODO: Max this limit
        let limit = query.limit.0 as usize;
        let mut rng = rand::thread_rng();
        let now = std::time::UNIX_EPOCH
            .elapsed()
            .expect("Bootstrap server time set before epoch")
            .as_millis();
        self.0
            .read()
            .get(query.space.as_ref())
            .map(|space| {
                space
                    .values()
                    .filter(|i| i.expires_at_ms as u128 > now)
                    .filter_map(|i| {
                        let mut buf = Vec::new();
                        match rmp_encode(&mut buf, i) {
                            Ok(_) => Some(buf),
                            Err(_) => None,
                        }
                    })
                    .choose_multiple(&mut rng, limit)
            })
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        self.0.write().clear()
    }

    #[cfg(test)]
    pub fn all(&self) -> HashMap<KitsuneSpace, HashMap<KitsuneAgent, AgentInfoSigned>> {
        self.0.read().clone()
    }
}
