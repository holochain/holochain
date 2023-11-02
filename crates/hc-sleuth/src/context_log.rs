use std::{collections::HashSet, hash::Hash, io::BufRead, sync::Arc};

use aitia::{
    cause::FactTraits,
    logging::{Log, LogLine},
    Fact,
};
use holochain_p2p::DnaHashExt;
use holochain_types::prelude::*;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

use super::*;

pub type ContextWriter = aitia::logging::LogWriter<Context>;

// #[derive(Debug, derive_more::From)]
pub type CtxError = String;
pub type ContextResult<T> = Result<T, CtxError>;

pub fn init_subscriber() -> ContextWriter {
    let w = ContextWriter::default();
    let ww = w.clone();
    tracing_subscriber::registry()
        .with(holochain_trace::standard_layer(std::io::stderr).unwrap())
        .with(aitia::logging::tracing_layer(move || ww.clone()))
        .init();
    w
}

#[derive(Default, Debug)]
pub struct Context {
    /// All steps recorded
    pub facts: HashSet<Step>,

    ///
    pub entry_actions: HashMap<EntryHash, ActionHash>,
    pub map_node_to_agents: HashMap<SleuthId, HashSet<AgentPubKey>>,
    pub map_agent_to_node: HashMap<AgentPubKey, SleuthId>,
    pub map_op_to_sysval_dep_hash: HashMap<OpRef, Option<ActionHash>>,
    pub map_op_to_appval_dep_hash: HashMap<OpRef, HashSet<AnyDhtHash>>,
    pub map_dep_hash_to_op: HashMap<AnyDhtHash, OpRef>,
    pub map_action_to_op: HashMap<OpAction, OpRef>,
    pub op_info: HashMap<OpRef, OpInfo>,
}

impl Context {
    pub fn from_file(mut r: impl BufRead) -> Self {
        use aitia::logging::Log;
        let mut la = Self::default();
        let mut line = String::new();
        while let Ok(_) = r.read_line(&mut line) {
            if let Some(fact) = Self::parse(&line) {
                la.apply(fact);
            }
        }
        la
    }

    pub fn check(&self, fact: &Step) -> bool {
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
    pub fn sysval_op_dep(&self, op: &OpRef) -> ContextResult<Option<&OpInfo>> {
        self.map_op_to_sysval_dep_hash
            .get(op)
            .ok_or(format!("map_op_to_sysval_dep_hash({op})"))?
            .as_ref()
            .map(|h| {
                self.map_dep_hash_to_op
                    .get(&h.clone().into())
                    .ok_or(format!("map_dep_hash_to_op({h})"))
            })
            .transpose()?
            .map(|d| self.op_info(d))
            .transpose()
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

    pub fn op_to_action(&self, op: &OpRef) -> ContextResult<OpAction> {
        Ok(OpAction::from((**self.op_info(op)?).clone())).into()
    }

    pub fn op_from_action(&self, action: ActionHash, op_type: DhtOpType) -> ContextResult<OpRef> {
        let oa = OpAction(action, op_type);
        self.map_action_to_op
            .get(&oa)
            .cloned()
            .ok_or(format!("map_action_to_op({oa:?})"))
    }
}

impl aitia::logging::Log for Context {
    type Fact = Step;

    fn apply(&mut self, fact: Step) {
        match fact.clone() {
            Step::Published { by, op } => {}
            Step::Integrated { by, op } => {}
            Step::AppValidated { by, op } => {}
            Step::SysValidated { by, op } => {}
            Step::PendingAppValidation { by, op, deps } => {
                self.map_op_to_appval_dep_hash
                    .entry(op)
                    .or_default()
                    .extend(deps.into_iter());
            }
            Step::Fetched { by, op } => {}
            Step::Authored { by: _, op } => {
                // TODO: add check that the same op is not authored twice?
                let op_hash = op.as_hash();
                let a = OpAction::from((*op).clone());
                self.map_dep_hash_to_op
                    .insert(op.fetch_dependency_hash(), op_hash.clone());
                self.map_action_to_op.insert(a, op_hash.clone());
                self.map_op_to_sysval_dep_hash
                    .insert(op_hash.clone(), op.dep.clone());
                self.op_info.insert(op_hash.clone(), op);
            }
            Step::AgentJoined { node, agent } => {
                self.map_agent_to_node.insert(agent.clone(), node.clone());
                self.map_node_to_agents
                    .entry(node)
                    .or_default()
                    .insert(agent);
            }
        }
        let duplicate = self.facts.insert(fact.clone());
        if duplicate {
            tracing::warn!("Duplicate fact {:?}", fact);
        }
    }
}
