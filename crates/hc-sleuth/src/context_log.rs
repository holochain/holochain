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

#[derive(Debug)]
pub struct CtxError;
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
    facts: HashSet<Step>,
    pub(crate) node_agents: HashMap<SleuthId, HashSet<AgentPubKey>>,
    entry_actions: HashMap<EntryHash, ActionHash>,
    map_op_to_sysval_dep_hash: HashMap<OpRef, Option<AnyDhtHash>>,
    map_op_to_appval_dep_hash: HashMap<OpRef, HashSet<AnyDhtHash>>,
    map_dep_hash_to_op: HashMap<AnyDhtHash, OpRef>,
    map_action_to_op: HashMap<OpAction, OpRef>,
    op_info: HashMap<OpRef, OpInfo>,
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

    pub fn sysval_op_dep(&self, op: &OpRef) -> ContextResult<Option<&OpInfo>> {
        self.map_op_to_sysval_dep_hash
            .get(op)
            .ok_or(CtxError)?
            .as_ref()
            .map(|o| self.map_dep_hash_to_op.get(o).ok_or(CtxError))
            .transpose()?
            .map(|o| self.op_info.get(o).ok_or(CtxError))
            .transpose()
    }

    pub fn appval_op_deps(&self, op: &OpRef) -> ContextResult<HashSet<&OpInfo>> {
        self.map_op_to_appval_dep_hash
            .get(op)
            .ok_or(CtxError)?
            .iter()
            .map(|o| self.map_dep_hash_to_op.get(o).ok_or(CtxError))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|o| self.op_info.get(o).ok_or(CtxError))
            .collect()
    }

    pub fn op_info(&self, op: &OpRef) -> ContextResult<&OpInfo> {
        self.op_info.get(op).ok_or(CtxError)
    }

    pub fn op_to_action(&self, op: &OpRef) -> ContextResult<OpAction> {
        Ok(OpAction::from((**self.op_info(op)?).clone())).into()
    }

    pub fn op_from_action(&self, action: ActionHash, op_type: DhtOpType) -> ContextResult<OpRef> {
        self.map_action_to_op
            .get(&OpAction(action, op_type))
            .cloned()
            .ok_or(CtxError)
    }
}

impl aitia::logging::Log for Context {
    type Fact = Step;

    fn apply(&mut self, fact: Step) {
        match fact.clone() {
            Step::Authored { by, action } => {}
            Step::Published { by, op } => {}
            Step::Integrated { by, op } => {}
            Step::AppValidated { by, op } => {}
            Step::SysValidated { by, op } => {}
            Step::PendingSysValidation { by, op, dep } => {
                self.map_op_to_sysval_dep_hash.insert(op, dep);
            }
            Step::PendingAppValidation { by, op, deps } => {
                self.map_op_to_appval_dep_hash
                    .entry(op)
                    .or_default()
                    .extend(deps.into_iter());
            }
            Step::Fetched { by, op } => {}
            Step::Seen { op } => {
                self.map_dep_hash_to_op
                    .insert(op.fetch_dependency_hash(), op.as_hash().clone());
                self.op_info.insert(op.as_hash().clone(), op);
            }
        }
        self.facts.insert(fact);
    }
}
