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
    sysval_dep: HashMap<OpAction, Option<OpAction>>,
    appval_deps: HashMap<OpAction, HashSet<OpAction>>,
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

    pub fn sysval_dep(&self, op: &OpAction) -> Option<&OpAction> {
        self.sysval_dep.get(op)?.as_ref()
    }

    pub fn appval_deps(&self, op: &OpAction) -> Option<&HashSet<OpAction>> {
        self.appval_deps.get(op)
    }

    pub fn node_ids(&self) -> &HashSet<String> {
        &self.node_ids
    }
}

impl aitia::logging::Log for Context {
    type Fact = Step;

    fn apply(&mut self, fact: Step) {
        self.facts.insert(fact);
    }
}
