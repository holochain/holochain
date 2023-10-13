use std::collections::{HashMap, HashSet};

use anyhow::bail;
use itertools::Itertools;

use crate::Fallible;

use super::{Crate, DependenciesT};

#[derive(Debug)]
struct Node<'a> {
    c: &'a Crate<'a>,
    direct_ws_deps: HashSet<cargo::core::Dependency>,
    in_degree: usize,
    out_degree: usize,
}

// Based on Kahn’s algorithm for topological sorting of a graph and does not assume that the graph is connected
pub fn flatten_forest<'a>(crates: &'a Vec<Crate<'a>>) -> Fallible<Vec<&'a Crate<'a>>> {
    // A set of nodes which track the degree of each node
    let mut forest = build_forest(crates);

    let mut order = Vec::<&'a Crate<'a>>::new();

    loop {
        let mut working_order = Vec::<(&'a Crate<'a>, HashSet<cargo::core::Dependency>)>::new();
        for node in forest.values() {
            if node.in_degree == 0 {
                working_order.push((node.c, node.direct_ws_deps.clone()));
            }
        }

        if working_order.is_empty() {
            break;
        }

        // Remove all the nodes from the forest which had no references to them in this pass
        for (c, deps) in &working_order {
            for d in deps {
                if let Some(n) = forest.get_mut(&d.package_name().to_string()) {
                    n.in_degree -= 1
                }
            }

            forest.remove(&c.name());
        }

        // Push those same nodes to the final list
        order.extend(
            working_order
                .into_iter()
                .map(|(c, _)| c)
                .sorted_by(|a, b| a.name().cmp(&b.name())),
        );
    }

    if crates.len() != order.len() {
        bail!("While attempting to order crates, managed to order {} of {} crates. This likely means there is a cyclic dependency.", order.len(), crates.len());
    }

    Ok(order.into_iter().rev().collect())
}

fn build_forest<'a>(crates: &'a Vec<Crate<'a>>) -> HashMap<String, Node> {
    let mut forest = HashMap::<String, Node>::new();
    for c in crates {
        let node = Node {
            c: &c,
            direct_ws_deps: HashSet::new(),
            in_degree: 0,
            out_degree: 0,
        };

        forest.insert(c.name(), node);
    }

    let workspace_packages: HashSet<String> = forest.iter().map(|(name, _)| name.clone()).collect();

    let mut in_degrees = HashMap::<String, usize>::new();

    for node in forest.values_mut() {
        let deps = node.c.direct_workspace_dependencies(&workspace_packages);
        node.direct_ws_deps = deps.iter().cloned().collect();
        node.out_degree = deps.len();
        for d in deps {
            let degree = in_degrees.entry(d.package_name().to_string()).or_insert(0);
            *degree += 1;
        }
    }

    for node in forest.values_mut() {
        if let Some(degree) = in_degrees.get(&node.c.name()) {
            node.in_degree = *degree;
        }
    }

    forest
}
