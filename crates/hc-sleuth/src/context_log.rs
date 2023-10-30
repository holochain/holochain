use std::{collections::HashSet, hash::Hash, io::BufRead, sync::Arc};

use aitia::{
    cause::FactTraits,
    logging::{Log, LogLine},
    Fact,
};
use holochain_p2p::DnaHashExt;

use super::*;

pub type ContextWriter = aitia::logging::AitiaWriter<Context>;

#[derive(Default, Debug)]
pub struct Context {
    facts: HashSet<Step>,
    node_ids: HashSet<String>,
    sysval_dep: HashMap<OpRef, Option<OpRef>>,
    appval_deps: HashMap<OpRef, Vec<OpRef>>,
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

    pub fn sysval_dep(&self, op: &OpRef) -> Option<&OpRef> {
        self.sysval_dep.get(op)?.as_ref()
    }

    pub fn appval_deps(&self, op: &OpRef) -> Option<&Vec<OpRef>> {
        self.appval_deps.get(op)
    }

    pub fn node_ids(&self) -> &HashSet<String> {
        &self.node_ids
    }
}

impl aitia::logging::Log for Context {
    type Fact = Step;

    fn parse(line: &str) -> Option<Step> {
        regex::Regex::new("<AITIA>(.*?)</AITIA>")
            .unwrap()
            .captures(line)
            .and_then(|m| m.get(1))
            .map(|m| Step::decode(m.as_str()))
    }

    fn apply(&mut self, fact: Step) {
        self.facts.insert(fact);
    }
}
