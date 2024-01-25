use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

use crate::{Dep, Fact};

#[derive(Debug, derive_more::From, derive_more::Deref, derive_more::DerefMut)]
pub struct DepGraph<'c, T: Fact>(petgraph::graph::DiGraph<GraphNode<'c, T>, ()>);

#[derive(PartialEq, Eq, Hash)]
pub struct GraphNode<'c, T: Fact> {
    pub dep: Dep<T>,
    pub ctx: &'c T::Context,
}

impl<'c, T: Fact> Clone for GraphNode<'c, T> {
    fn clone(&self) -> Self {
        Self {
            dep: self.dep.clone(),
            ctx: self.ctx,
        }
    }
}

impl<'c, T: Fact> DepGraph<'c, T> {
    pub fn deps(&self) -> HashSet<Dep<T>> {
        self.node_weights()
            .map(|n| n.dep.clone())
            .collect::<HashSet<_>>()
    }

    pub fn leaves(&self) -> HashSet<&Dep<T>> {
        self.node_indices()
            .filter(|i| {
                self.edges_directed(*i, petgraph::Direction::Outgoing)
                    .count()
                    == 0
            })
            .filter_map(|i| self.node_weight(i))
            .map(|n| &n.dep)
            .collect()
    }

    pub fn report(&self) -> std::io::Result<String> {
        use std::fmt::Write;
        let mut out = "".to_string();
        let dot = format!(
            "{:?}",
            petgraph::dot::Dot::with_attr_getters(
                &**self,
                &[petgraph::dot::Config::EdgeNoLabel],
                &|_g, _e| "".to_string(),
                &|_g, _n| { "nojustify=true".to_string() },
            )
        );

        if let Ok(graph) = graph_easy(&dot) {
            writeln!(&mut out, "Original dot output:\n\n{}", dot).unwrap();
            writeln!(&mut out, "`graph-easy` output:\n{}", graph).unwrap();
        } else {
            writeln!(
                &mut out,
                "`graph-easy` not installed. Original dot output:\n\n{}",
                dot
            )
            .unwrap();
        }
        Ok(out)
    }

    pub fn print(&self) {
        let report = self.report().unwrap();
        println!("{report}");
    }
}

impl<'c, T: Fact> Debug for GraphNode<'c, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.dep.explain(self.ctx))
    }
}

impl<'c, T: Fact> Default for DepGraph<'c, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

/// If a `graph-easy` binary is installed, render an ASCII graph from the
/// provided dot syntax.
pub fn graph_easy(dot: &str) -> anyhow::Result<String> {
    use std::io::{Read, Write};

    let process = std::process::Command::new("graph-easy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;

    process.stdin.unwrap().write_all(dot.as_bytes()).unwrap();
    let mut s = String::new();
    process.stdout.unwrap().read_to_string(&mut s).unwrap();

    Ok(s)
}
