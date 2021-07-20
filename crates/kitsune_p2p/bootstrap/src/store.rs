use std::{collections::HashMap, sync::Arc};

use kitsune_p2p_types::{
    agent_info::AgentInfoSigned,
    bin_types::{KitsuneAgent, KitsuneSpace},
    bootstrap::RandomQuery,
    codec::rmp_encode,
};
use parking_lot::RwLock;
use rand::seq::IteratorRandom;

type AgentMap = HashMap<Arc<KitsuneAgent>, AgentInfoSigned>;
type SpaceMap = HashMap<Arc<KitsuneSpace>, AgentMap>;

#[derive(Clone, Debug)]
pub(crate) struct Store(Arc<RwLock<SpaceMap>>);

impl Store {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn prune(&self) {
        let now = std::time::UNIX_EPOCH
            .elapsed()
            .expect("Bootstrap server time set before epoch")
            .as_millis() as u64;

        self.0.write().retain(|_, map| {
            map.retain(|_, info| info.expires_at_ms >= now);
            !map.is_empty()
        });
    }

    pub fn put(&self, info: AgentInfoSigned) {
        let mut lock = self.0.write();
        let space_map = lock.entry(info.space.clone()).or_insert_with(HashMap::new);
        match space_map.entry(info.agent.clone()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if info.signed_at_ms > e.get().signed_at_ms {
                    e.insert(info);
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(info);
            }
        }
    }

    pub fn random(&self, query: RandomQuery) -> Vec<Vec<u8>> {
        // TODO: Max this limit
        let limit = query.limit.0 as usize;
        let mut rng = rand::thread_rng();
        let now = std::time::UNIX_EPOCH
            .elapsed()
            .expect("Bootstrap server time set before epoch")
            .as_millis() as u64;
        self.0
            .read()
            .get(query.space.as_ref())
            .map(|space| {
                space
                    .values()
                    .filter_map(|i| {
                        if i.expires_at_ms <= now {
                            return None;
                        }
                        if i.url_list.is_empty() {
                            return None;
                        }
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
    pub fn all(&self) -> HashMap<Arc<KitsuneSpace>, HashMap<Arc<KitsuneAgent>, AgentInfoSigned>> {
        self.0.read().clone()
    }
}
