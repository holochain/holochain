use std::{collections::HashSet, io::BufRead, sync::Arc};

use holochain_types::prelude::*;
use once_cell::sync::OnceCell;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

use super::*;

pub type ContextSubscriber = aitia::logging::AitiaSubscriber<Context>;

// #[derive(Debug, derive_more::From)]
pub type CtxError = String;
pub type ContextResult<T> = Result<T, CtxError>;

pub fn init_subscriber() -> ContextSubscriber {
    let w = SUBSCRIBER.get_or_init(ContextSubscriber::default).clone();
    let ww = w.clone();
    tracing_subscriber::registry()
        .with(holochain_trace::standard_layer(std::io::stderr).unwrap())
        .with(aitia::logging::tracing_layer(move || ww.clone()))
        .init();
    w
}

pub static SUBSCRIBER: OnceCell<ContextSubscriber> = OnceCell::new();

#[derive(Default, Debug)]
pub struct Context {
    /// All events ever recorded
    pub events: Vec<(Timestamp, Arc<Fact>)>,

    /// All facts ever recorded
    pub facts: HashSet<Arc<Fact>>,

    /// Track which agents are part of which nodes
    pub map_node_to_agents: HashMap<SleuthId, HashSet<AgentPubKey>>,

    /// Track which node an agent is part of
    pub map_agent_to_node: HashMap<AgentPubKey, SleuthId>,

    /// Track the sys validation deps for an op hash
    pub map_op_to_sysval_dep_hashes: HashMap<OpRef, Vec<ActionHash>>,

    /// Track the app validation deps for an op hash
    pub map_op_to_appval_dep_hash: HashMap<OpRef, HashSet<AnyDhtHash>>,

    /// Track which op a dependency is part of
    pub map_dep_hash_to_op: HashMap<AnyDhtHash, OpRef>,

    /// Map the action hash to the associated op hashes
    pub map_action_to_ops: HashMap<ActionHash, HashSet<OpRef>>,

    /// Map the warrant hash to the op hashes
    pub map_warrant_to_ops: HashMap<WarrantHash, HashSet<OpRef>>,

    /// The full info associated with each op hash
    pub op_info: HashMap<OpRef, OpInfo>,
}

impl Context {
    pub fn from_file(r: impl BufRead) -> Self {
        let mut la = Self::default();
        la.apply_log(r);
        la
    }

    pub fn apply_log(&mut self, r: impl BufRead) {
        use aitia::logging::Log;
        for line in r.lines() {
            if let Some(fact) = Self::parse(&line.unwrap()) {
                self.apply(fact);
            }
        }
    }

    pub fn check(&self, fact: &Fact) -> bool {
        self.facts.contains(fact)
    }

    pub fn node_agents(&self, id: &SleuthId) -> ContextResult<&HashSet<AgentPubKey>> {
        self.map_node_to_agents
            .get(id)
            .ok_or(format!("node_agents({id})"))
    }

    pub fn agent_node(&self, agent: &AgentPubKey) -> ContextResult<&SleuthId> {
        self.map_agent_to_node
            .get(agent)
            .ok_or(format!("agent_node({agent})"))
    }

    /// Get the sys validation dependency of this op hash if applicable
    pub fn sysval_op_deps(&self, op: &OpRef) -> ContextResult<Vec<&OpInfo>> {
        self.map_op_to_sysval_dep_hashes
            .get(op)
            .ok_or(format!("map_op_to_sysval_dep_hash({op})"))?
            .iter()
            .map(|h| {
                self.map_dep_hash_to_op
                    .get(&h.clone().into())
                    .ok_or(format!("map_dep_hash_to_op({h})"))
                    .and_then(|d| self.op_info(d))
            })
            .collect::<Result<Vec<_>, _>>()
    }

    /// Get the app validation dependencies of this op hash
    pub fn appval_op_deps(&self, op: &OpRef) -> ContextResult<HashSet<&OpInfo>> {
        self.map_op_to_appval_dep_hash
            .get(op)
            .ok_or(format!("map_op_to_appval_dep_hash({op})"))?
            .iter()
            .map(|h| {
                self.map_dep_hash_to_op
                    .get(h)
                    .ok_or(format!("map_dep_hash_to_op({h})"))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|d| self.op_info(d))
            .collect()
    }

    pub fn op_info(&self, op: &OpRef) -> ContextResult<&OpInfo> {
        self.op_info.get(op).ok_or(format!("op_info({op})"))
    }

    pub fn ops_from_action(&self, action: &ActionHash) -> ContextResult<HashSet<OpRef>> {
        self.map_action_to_ops
            .get(action)
            .cloned()
            .ok_or(format!("map_action_to_ops({action:?})"))
    }

    pub fn as_if(&mut self) {
        todo!()
    }

    pub fn all_events_for_topic() {}
}

impl aitia::logging::Log for Context {
    type Event = Event;

    fn apply(&mut self, event: Event) {
        match event.fact.clone() {
            Fact::Integrated { op, .. } => {
                debug_assert!(self.op_info.contains_key(&op))
            }
            Fact::AppValidated { op, .. } => {
                debug_assert!(self.op_info.contains_key(&op))
            }
            Fact::SysValidated { op, .. } => {
                debug_assert!(self.op_info.contains_key(&op))
            }
            Fact::MissingAppValDep { by: _, op, deps } => {
                debug_assert!(self.op_info.contains_key(&op));
                self.map_op_to_appval_dep_hash
                    .entry(op)
                    .or_default()
                    .extend(deps);
            }
            Fact::Fetched { op, .. } => {
                debug_assert!(self.op_info.contains_key(&op))
            }
            Fact::ReceivedHash { op, .. } => {
                debug_assert!(self.op_info.contains_key(&op))
            }
            Fact::SentHash { op, .. } => {
                debug_assert!(self.op_info.contains_key(&op))
            }
            Fact::Authored { by: _, op } => {
                // TODO: add check that the same op is not authored twice?
                let op_hash = op.as_hash();
                match &op.op {
                    DhtOpLite::Chain(op) => {
                        self.map_action_to_ops
                            .entry(op.action_hash().clone())
                            .or_default()
                            .insert(op_hash.clone());
                    }
                    DhtOpLite::Warrant(op) => {
                        self.map_warrant_to_ops
                            .entry(op.warrant().to_hash())
                            .or_default()
                            .insert(op_hash.clone());
                    }
                };
                for h in op.fetch_dependency_hashes() {
                    self.map_dep_hash_to_op.insert(h, op_hash.clone());
                }
                self.map_op_to_sysval_dep_hashes
                    .insert(op_hash.clone(), op.dep.clone());
                self.op_info.insert(op_hash.clone(), op);
            }
            Fact::AgentJoined { node, agent } => {
                self.map_agent_to_node.insert(agent.clone(), node.clone());
                self.map_node_to_agents
                    .entry(node)
                    .or_default()
                    .insert(agent);
            }
            Fact::SweetConductorShutdown { node } => {
                if let Some(agents) = self.map_node_to_agents.remove(&node) {
                    for a in agents {
                        self.map_agent_to_node.remove(&a);
                    }
                }
            }
        }

        let timestamp = event.timestamp;
        let fact = Arc::new(event.fact);

        let duplicate = self.facts.insert(fact.clone());
        if duplicate {
            tracing::warn!("Duplicate fact {:?}", fact);
        }
        self.events.push((timestamp, fact));
    }
}
