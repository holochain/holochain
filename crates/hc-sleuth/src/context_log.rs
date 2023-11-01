use std::{collections::HashSet, hash::Hash, io::BufRead, sync::Arc};

use aitia::{
    cause::FactTraits,
    logging::{Log, LogLine},
    Fact,
};
use holochain_p2p::DnaHashExt;
use holochain_state::prelude::hash_type::AnyDht;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

use super::*;

pub type ContextWriter = aitia::logging::LogWriter<Context>;

pub type ContextError = ();
pub type ContextResult<T> = Result<T, ContextError>;

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
    node_ids: HashSet<String>,
    entry_actions: HashMap<EntryHash, ActionHash>,
    map_action_to_sysval_fetch_hash: HashMap<OpAction, Option<AnyDhtHash>>,
    map_action_to_appval_fetch_hash: HashMap<OpAction, HashSet<AnyDhtHash>>,
    map_fetch_hash_to_op: HashMap<AnyDhtHash, OpLite>,
    map_action_to_op: HashMap<OpAction, OpLite>,
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

    pub fn sysval_op_dep(&self, op: &OpAction) -> ContextResult<Option<OpLite>> {
        self.map_action_to_sysval_fetch_hash
            .get(op)
            .ok_or(())?
            .as_ref()
            .map(|o| self.map_fetch_hash_to_op.get(o).cloned().ok_or(()))
            .transpose()
    }

    pub fn appval_op_deps(&self, op: &OpAction) -> ContextResult<HashSet<OpLite>> {
        self.map_action_to_appval_fetch_hash
            .get(op)
            .ok_or(())?
            .iter()
            .map(|o| self.map_fetch_hash_to_op.get(o).cloned().ok_or(()))
            .collect::<Result<HashSet<OpLite>, _>>()
    }

    pub fn action_to_op(&self, op: &OpAction) -> ContextResult<OpLite> {
        self.map_action_to_op.get(op).cloned().ok_or(())
    }

    pub fn node_ids(&self) -> &HashSet<String> {
        &self.node_ids
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
                self.map_action_to_sysval_fetch_hash.insert(op, dep);
            }
            Step::PendingAppValidation { by, op, deps } => {
                self.map_action_to_appval_fetch_hash
                    .entry(op)
                    .or_default()
                    .extend(deps.into_iter());
            }
            Step::Fetched { by, op } => {}
            Step::Seen { by, op_lite } => {
                self.map_fetch_hash_to_op
                    .insert(op_lite.fetch_dependency_hash(), op_lite.clone());
                self.map_action_to_op
                    .insert(OpAction::from(op_lite.clone()), op_lite);
            }
        }
        self.facts.insert(fact);
    }
}
