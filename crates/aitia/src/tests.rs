use std::collections::HashSet;

use maplit::hashset;

use super::simple_report as report;
use crate::dep::*;
use crate::graph::*;

fn path_lengths<T: Fact>(graph: &DepTree<T>, start: Dep<T>, end: Dep<T>) -> Vec<usize> {
    let start_ix = graph
        .node_indices()
        .find(|i| graph[*i].dep == start)
        .unwrap();
    let end_ix = graph
        .node_indices()
        .find(|i| graph[*i].dep == end)
        .unwrap();
    petgraph::algo::all_simple_paths::<Vec<_>, _>(&**graph, start_ix, end_ix, 0, None)
        .map(|c| c.len())
        .collect()
}

type Checks<T> = Box<dyn Fn(&T) -> bool>;


/// Tests exploring all the possible graphs involving a single node
mod singleton {
    use super::*;
    use test_case::test_case;

    #[derive(Clone, Debug, PartialEq, Eq, Hash)]
    struct Singleton(bool, bool);
    
    impl Fact for Singleton {
        type Context = ();
    
        fn check(&self, (): &Self::Context) -> bool {
            self.0
        }
    
        fn dep(&self, (): &Self::Context) -> DepResult<Self> {
            Ok(self.1.then_some(self.clone().into()))
        }
    }
    
    #[test_case( Singleton(false, false) => Some(hashset! { Singleton(false, false).into() }) ; "failing single isolated fact produces only self")]
    #[test_case( Singleton(false, true)  => Some(hashset! { Singleton(false, true).into() }) ; "failing single self-referencing fact produces only self")]
    #[test_case( Singleton(true, false) => None ; "passing single isolated fact produces Pass")]
    #[test_case( Singleton(true, true) => None  ; "passing single self-referencing fact produces Pass")]
    fn singleton_deps(fact: Singleton) -> Option<HashSet<Dep<Singleton>>> {
        holochain_trace::test_run().ok().unwrap();
        Dep::from(fact.clone())
            .traverse(&())
            .fail()
            .map(|(graph, _)| graph.deps())
    }
}

/// Tests exploring a single, acyclic path with various starting and ending points
mod acyclic_single_path {
    use super::*;
    use test_case::test_case;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Countdown(u8);
    
    
    impl Fact for Countdown {
        type Context = Checks<Self>;
    
        fn dep(&self, _: &Self::Context) -> DepResult<Self> {
            Ok(match self.0 {
                3 => Some(Self(2).into()),
                2 => Some(Self(1).into()),
                1 => Some(Self(0).into()),
                0 => None,
                _ => unreachable!(),
            })
        }
    
        fn check(&self, ctx: &Self::Context) -> bool {
            (ctx)(self)
        }
    }
    
    #[test_case( 
        Countdown(3), 
        None
        => hashset![3, 2, 1, 0]
        ; "Countdown from 3 with all false returns path from 3 to 0"
    )]
    #[test_case( 
        Countdown(2), 
        Some(3) 
        => hashset![2, 1, 0]
        ; "Countdown from 2 with 3 being true returns path from 2 to 0"
    )]
    #[test_case( 
        Countdown(3), 
        Some(0) 
        => hashset![3, 2, 1]
        ; "Countdown from 3 with 0 being true returns path from 3 to 1"
    )]
    #[test_case( 
        Countdown(3), 
        Some(1) 
        => hashset![3, 2]
        ; "Countdown from 3 with 1 being true returns path from 3 to 2"
    )]
    #[test_case( 
        Countdown(1), 
        Some(2) 
        => hashset![1, 0]
        ; "Countdown from 1 with 3 being true returns path from 1 to 0"
    )]
    fn single_path(countdown: Countdown, true_one: Option<u8>) -> HashSet<u8> {
        holochain_trace::test_run().ok().unwrap();
    
        let checker: Checks<Countdown> = Box::new(move |c| Some(c.0) == true_one);
        let tr = Dep::from(countdown).traverse(&checker);
        report(&tr);
        let (graph, _passes) = tr
            .fail()
            .unwrap();
    
        let nodes: HashSet<_> = graph.deps().into_iter().map(|c| match c {
            Dep::Fact(f) => f.0,
            _ => unreachable!(),
        }).collect();
        
        let edges = graph.edge_count();
    
        // If the number of edges is one less than the number of nodes, that implies a straight noncyclic path
        // (this doesn't test that something weird and ridiculous happens like a branch with a disconnected node)
        assert_eq!(nodes.len(), edges + 1);
        nodes
    }
}

/// Tests exploring a single loop with various starting and ending points
mod single_loop {
    use super::*;
    use test_case::test_case;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Countdown(u8);

    impl Fact for Countdown {
        type Context = Checks<Self>;

        fn dep(&self, _: &Self::Context) -> DepResult<Self> {
            Ok(match self.0 {
                3 => Some(Self(2).into()),
                2 => Some(Self(1).into()),
                1 => Some(Self(3).into()),
                0 => None,
                _ => unreachable!(),
            })
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            (ctx)(self)
        }
    }

    #[test_case( 
        Countdown(3), 
        Some(0)
        => (hashset![3, 2, 1], 3)
        ; "Countdown from 3 with all in loop false returns entire loop"
    )]    
    #[test_case( 
        Countdown(1), 
        Some(0)
        => (hashset![3, 2, 1], 3)
        ; "Countdown from 1 with all in loop false returns entire loop"
    )]
    #[test_case( 
        Countdown(1), 
        Some(2)
        => (hashset![1, 3], 1)
        ; "Countdown from 1 with 2 true returns path 1->3"
    )]    
    #[test_case( 
        Countdown(2), 
        Some(3)
        => (hashset![2, 1], 1)
        ; "Countdown from 2 with 3 true returns path 2->1"
    )]
    fn single_loop(countdown: Countdown, true_one: Option<u8>) -> (HashSet<u8>, usize) {
        holochain_trace::test_run().ok().unwrap();
    
        let checker: Checks<Countdown> = Box::new(move |c| Some(c.0) == true_one);
        let tr = Dep::from(countdown).traverse(&checker);
        report(&tr);
        let (graph, _passes) = tr
            .fail()
            .unwrap();
    
        let nodes: HashSet<_> = graph.deps().into_iter().map(|c| match c {
            Dep::Fact(f) => f.0,
            _ => unreachable!(),
        }).collect();
        
        let num_edges = graph.edge_count();
    
        (nodes, num_edges)
    }

    
}

/// Contrived test case involving graphs mostly consisting of ANY nodes
#[test]
fn branching_any() {
    holochain_trace::test_run().ok().unwrap();

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Branching(u8);

    impl Fact for Branching {
        type Context = HashSet<u8>;

        fn dep(&self, _ctx: &Self::Context) -> DepResult<Self> {
            // If greater than 64, then the node is a leaf of the tree
            // Otherwise, each node `n` branches off to the other numbers `i` where `floor(i / 2) = n`
            Ok((self.0 <= 64)
                .then(|| Dep::any(vec![Self(self.0 * 2).into(), Self(self.0 * 2 + 1).into()])))
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            ctx.contains(&self.0)
        }
    }

    {
        let ctx = maplit::hashset![40, 64];
        let tr = Dep::from(Branching(2)).traverse(&ctx);
        // report(&tr);
        let (graph, _passes) = tr.fail().unwrap();
        assert_eq!(
            path_lengths(&graph, Branching(2).into(), Branching(20).into()),
            vec![7]
        );
        assert_eq!(
            path_lengths(&graph, Branching(2).into(), Branching(32).into()),
            vec![9]
        );
    }
    {
        let ctx = (32..128).collect();
        let tr = Dep::from(Branching(2)).traverse(&ctx);
        // report(&tr);
        let _ = tr.fail().unwrap();
        // no assertion here, just a smoke test. It's a neat case, check the graph output
    }
}


/// Emulating a recipe for a tuna melt sandwich to illustrate functionality of EVERY nodes
mod recipes {

    use super::*;
    use test_case::test_case;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    enum Recipe {
        Eggs,
        Vinegar,
        Mayo,
        Tuna,
        Cheese,
        TunaSalad,
        Bread,
        TunaMelt,
        GrilledCheese,
    }

    use Recipe::*;

    impl Fact for Recipe {
        type Context = HashSet<Recipe>;

        fn dep(&self, _ctx: &Self::Context) -> DepResult<Self> {
            use Recipe::*;
            Ok(match self {
                Eggs => None,
                Vinegar => None,
                Mayo => Some(Dep::every(vec![Eggs.into(), Vinegar.into()])),
                Tuna => None,
                Cheese => None,
                Bread => None,
                TunaSalad => Some(Dep::every(vec![Tuna.into(), Mayo.into()])),
                TunaMelt => Some(Dep::every(vec![
                    TunaSalad.into(),
                    Cheese.into(),
                    Bread.into(),
                ])),
                GrilledCheese => Some(Dep::every(vec![Cheese.into(), Bread.into()])),
            })
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            ctx.contains(&self)
        }
    }

    #[test_case(
        GrilledCheese, hashset![Bread, Cheese]
        => hashset![GrilledCheese]
        ; "GrilledCheese can be made with just Cheese and Bread"
    )]
    #[test_case(
        GrilledCheese, hashset![Bread]
        => hashset![Cheese]
        ; "GrilledCheese can't be made without Cheese"
    )]
    #[test_case(
        TunaMelt, hashset![Bread, Cheese]
        => hashset![Tuna, Vinegar, Eggs]
        ; "TunaMelt requires other essential ingredients"
    )]    
    #[test_case(
        TunaMelt, hashset![Bread, Cheese, TunaSalad]
        => hashset![TunaMelt]
        ; "TunaMelt can be made with these ingredients"
    )]    
    #[test_case(
        TunaMelt, hashset![Bread, Cheese, Eggs, Tuna]
        => hashset![Vinegar]
        ; "TunaMelt only requires vinegar"
    )]    
    #[test_case(
        TunaMelt, hashset![Bread, Cheese, Mayo]
        => hashset![Tuna]
        ; "TunaMelt only requires tuna"
    )]
    fn every_dep_simple(item: Recipe, truths: HashSet<Recipe>) -> HashSet<Recipe> {
        holochain_trace::test_run().ok().unwrap();

        // TODO:
        // - loops with Every might take some more thought.

        let tr = Dep::from(item).traverse(&truths);
        report(&tr);
        let (g, _) = tr.fail().unwrap();
        g.leaves().into_iter().map(|c| c.clone().into_fact().unwrap()).collect()
    }


}


/// A test similar to what Holochain uses, since that's what this lib was written for
#[test]
#[ignore = "should be run manually"]
fn holochain_like() {
    holochain_trace::test_run().ok().unwrap();

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
    struct F {
        which: bool,
        stage: Stage,
    }

    impl std::fmt::Display for F {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let who = match self.which {
                false => "Fatma",
                true => "Trudy",
            };
            f.write_fmt(format_args!("{} {:?}", who, self.stage))
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Stage {
        Create,
        Fetch,
        ReceiveA,
        ReceiveB,
        Store,
        SendA,
        SendB,
    }

    impl Fact for F {
        type Context = Checks<Self>;

        fn dep(&self, _ctx: &Self::Context) -> DepResult<Self> {
            use Stage::*;
            Ok(match self.stage {
                Create => None,
                Fetch => Some(Dep::any_named(
                    "Receive either",
                    vec![self.mine(ReceiveA), self.mine(ReceiveB)],
                )),
                ReceiveA => Some(self.theirs(SendA)),
                ReceiveB => Some(self.theirs(SendB)),
                Store => Some(Dep::any_named(
                    "Hold",
                    vec![self.mine(Create), self.mine(Fetch)],
                )),
                SendA => Some(self.mine(Store)),
                SendB => Some(self.mine(Store)),
            })
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            (ctx)(self)
        }

        fn explain(&self, _ctx: &Self::Context) -> String {
            self.to_string()
        }
    }

    impl F {
        pub fn mine(&self, stage: Stage) -> Dep<Self> {
            Dep::Fact(Self {
                which: self.which,
                stage,
            })
        }

        pub fn theirs(&self, stage: Stage) -> Dep<Self> {
            Dep::Fact(Self {
                which: !self.which,
                stage,
            })
        }
    }

    let fatma_store = Dep::Fact(F {
        which: false,
        stage: Stage::Store,
    });

    let checks: Checks<F> = Box::new(|step: &F| match (step.which, step.stage) {
        (true, Stage::Create) => true,
        // (false, Stage::Create) => true,

        // this leads to Trudy Create in the graph, which is wrong
        // TODO: revisit pruning branches that terminate with false, including loops
        //       basically we only want to create edges on the way back up from either
        //       a None, or a Loop detection
        // (true, Stage::ReceiveA) => true,

        // TODO: if all are false, then that doesn't work either
        _ => false,
        // (true, Stage::Fetch) => todo!(),
        // (true, Stage::ReceiveB) => todo!(),
        // (true, Stage::Store) => todo!(),
        // (true, Stage::SendA) => todo!(),
        // (true, Stage::SendB) => todo!(),
        // (false, Stage::Create) => todo!(),
        // (false, Stage::Fetch) => todo!(),
        // (false, Stage::ReceiveA) => todo!(),
        // (false, Stage::ReceiveB) => todo!(),
        // (false, Stage::Store) => todo!(),
        // (false, Stage::SendA) => todo!(),
        // (false, Stage::SendB) => todo!(),
    });

    report(&fatma_store.traverse(&checks));
}
