use std::{collections::HashSet, hash::Hash, io::BufRead, sync::Arc};

use aitia::{
    cause::FactTraits,
    logging::{Log, LogLine},
    Fact,
};
use holochain_types::prelude::*;

use super::*;

#[derive(Default)]
pub struct Context {
    pub nodes: NodeGroup,
}

#[derive(Default, derive_more::Deref)]
pub struct NodeGroup {
    #[deref]
    pub envs: HashMap<NodeId, NodeEnv>,
    pub agent_map: HashMap<AgentPubKey, NodeId>,
}

impl NodeGroup {
    pub fn add(&mut self, id: NodeId, node: NodeEnv, agents: &[AgentPubKey]) {
        self.envs.insert(id.clone(), node);
        self.agent_map
            .extend(agents.iter().map(move |a| (a.clone(), id.clone())));
    }

    pub fn node(&self, agent: &AgentPubKey) -> Option<&NodeEnv> {
        let id = self.agent_map.get(agent)?;
        self.envs.get(id)
    }
}

pub struct NodeEnv {
    pub authored: DbWrite<DbKindAuthored>,
    pub cache: DbWrite<DbKindCache>,
    pub dht: DbWrite<DbKindDht>,
    pub peers: DbWrite<DbKindP2pAgents>,
    pub metrics: DbWrite<DbKindP2pMetrics>,
}

impl NodeEnv {
    pub fn integrated<R: Send + 'static>(
        &self,
        f: impl 'static + Clone + Send + FnOnce(Transaction) -> anyhow::Result<Option<R>>,
    ) -> anyhow::Result<Option<R>> {
        if let Some(r) = self.authored.test_read(f.clone())? {
            Ok(Some(r))
        } else {
            self.dht.test_read(f.clone())
        }
    }

    pub fn exists<R: Send + 'static>(
        &self,
        f: impl 'static + Clone + Send + FnOnce(Transaction) -> anyhow::Result<Option<R>>,
    ) -> anyhow::Result<Option<R>> {
        todo!()
    }
}

#[cfg(feature = "test_utils")]
impl NodeEnv {
    pub fn mem() -> Self {
        let dna = DnaHash::from_raw_32(vec![0; 32]);
        let adna = std::sync::Arc::new(dna.clone());
        Self {
            authored: DbWrite::test_in_mem(DbKindAuthored(adna.clone())).unwrap(),
            cache: DbWrite::test_in_mem(DbKindCache(adna.clone())).unwrap(),
            dht: DbWrite::test_in_mem(DbKindDht(adna.clone())).unwrap(),
            peers: DbWrite::test_in_mem(DbKindP2pAgents(dna.to_kitsune())).unwrap(),
            metrics: DbWrite::test_in_mem(DbKindP2pMetrics(dna.to_kitsune())).unwrap(),
        }
    }

    // pub fn test() -> Self {
    //     Self {
    //         authored: test_authored_db(),
    //         cache: test_cache_db(),
    //         dht: test_dht_db(),
    //         peers: test_p2p_agents_db(),
    //         metrics: test_p2p_metrics_db(),
    //     }
    // }
}
