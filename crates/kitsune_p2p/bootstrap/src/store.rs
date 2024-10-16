use std::{collections::HashMap, sync::Arc};

use kitsune_p2p_types::{
    bin_types::{KitsuneAgent, KitsuneSignature, KitsuneSpace},
    bootstrap::RandomQuery,
};
use parking_lot::RwLock;
use rand::seq::IteratorRandom;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq, Clone))]
pub(crate) struct StoreEntry {
    pub encoded: Vec<u8>,
    pub signature: Arc<KitsuneSignature>,
    pub space: Arc<KitsuneSpace>,
    pub agent: Arc<KitsuneAgent>,
    pub signed_at_ms: u64,
    pub expires_at_ms: u64,
}

impl StoreEntry {
    pub fn parse(encoded: Vec<u8>) -> Result<Self, std::io::Error> {
        let mut bytes: &[u8] = &encoded;
        let kitsune_p2p_types::agent_info::agent_info_helper::AgentInfoSignedEncode {
            agent,
            signature,
            agent_info,
        } = kitsune_p2p_types::codec::rmp_decode(&mut bytes)?;

        let mut bytes: &[u8] = &agent_info;
        let info: kitsune_p2p_types::agent_info::agent_info_helper::AgentInfoEncode =
            kitsune_p2p_types::codec::rmp_decode(&mut bytes)?;

        if agent != info.agent {
            return Err(std::io::Error::other(
                "signed inner agent does not match unsigned outer agent",
            ));
        }

        Ok(StoreEntry {
            encoded,
            signature,
            space: info.space,
            agent,
            signed_at_ms: info.signed_at_ms,
            expires_at_ms: info.signed_at_ms + info.expires_after_ms,
        })
    }
}

type AgentMap = HashMap<Arc<KitsuneAgent>, StoreEntry>;
type SpaceMap = HashMap<Arc<KitsuneSpace>, AgentMap>;

#[derive(Clone, Debug)]
pub(crate) struct Store(Arc<RwLock<SpaceMap>>, Arc<Vec<String>>);

impl Store {
    pub fn new(proxy_list: Vec<String>) -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())), Arc::new(proxy_list))
    }

    pub fn proxy_list(&self) -> Arc<Vec<String>> {
        self.1.clone()
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

    pub fn put(&self, entry: StoreEntry) {
        let mut lock = self.0.write();
        let space_map = lock.entry(entry.space.clone()).or_default();
        match space_map.entry(entry.agent.clone()) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if entry.signed_at_ms > e.get().signed_at_ms {
                    e.insert(entry);
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(entry);
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
                        Some(i.encoded.to_vec())
                    })
                    .choose_multiple(&mut rng, limit)
            })
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        self.0.write().clear()
    }

    #[cfg(test)]
    pub fn all(&self) -> HashMap<Arc<KitsuneSpace>, HashMap<Arc<KitsuneAgent>, StoreEntry>> {
        self.0.read().clone()
    }
}
