use std::{collections::HashSet, hash::Hash, io::BufRead, sync::Arc};

use aitia::{
    cause::FactTraits,
    logging::{Log, LogLine},
    Fact,
};
use holochain_p2p::DnaHashExt;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

use super::*;

pub type ContextWriter = aitia::logging::LogWriter<Context>;

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
    sysval_dep: HashMap<OpLite, Option<AnyDhtHash>>,
    appval_deps: HashMap<OpLite, HashSet<AnyDhtHash>>,
    ops_by_fetch_hash: HashMap<AnyDhtHash, OpLite>,
    ops_by_action: HashMap<OpAction, OpLite>,
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

    pub fn sysval_op_dep(&self, op: &OpLite) -> Option<&OpLite> {
        self.ops_by_fetch_hash
            .get(self.sysval_dep.get(op)?.as_ref()?)
    }

    pub fn appval_op_deps(&self, op: &OpLite) -> HashSet<&OpLite> {
        if let Some(deps) = self.appval_deps.get(op) {
            deps.into_iter()
                .map(|h| self.ops_by_fetch_hash.get(h).unwrap())
                .collect()
        } else {
            HashSet::new()
        }
    }

    pub fn node_ids(&self) -> &HashSet<String> {
        &self.node_ids
    }

    pub fn expand(&self, step: Step<OpAction>) -> Step<OpLite> {
        match step {
            Step::Authored { by, action } => Step::Authored { by, action },
            Step::Published { by, op } => Step::Published {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
            },
            Step::Integrated { by, op } => Step::Integrated {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
            },
            Step::AppValidated { by, op } => Step::AppValidated {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
            },
            Step::SysValidated { by, op } => Step::SysValidated {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
            },
            Step::PendingSysValidation { by, op, dep } => Step::PendingSysValidation {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
                dep,
            },
            Step::PendingAppValidation { by, op, deps } => Step::PendingAppValidation {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
                deps,
            },
            Step::Fetched { by, op } => Step::Fetched {
                by,
                op: self.ops_by_action.get(&op).unwrap().clone(),
            },
        }
    }
}

impl aitia::logging::Log for Context {
    type Fact = Step<OpLite>;

    fn apply(&mut self, fact: Step) {
        match fact.clone() {
            Step::Authored { by, action } => {}
            Step::Published { by, op } => {}
            Step::Integrated { by, op } => {}
            Step::AppValidated { by, op } => {}
            Step::SysValidated { by, op } => {}
            Step::PendingSysValidation { by, op, dep } => {
                self.ops_by_fetch_hash
                    .insert(op.fetch_dependency_hash(), op.clone());
                self.ops_by_action.insert(
                    OpAction(op.action_hash().clone(), op.get_type()),
                    op.clone(),
                );

                self.sysval_dep.insert(op, dep);
            }
            Step::PendingAppValidation { by, op, deps } => {
                self.appval_deps
                    .entry(op)
                    .or_default()
                    .extend(deps.into_iter());
            }
            Step::Fetched { by, op } => {}
        }
        self.facts.insert(fact);
    }
}
