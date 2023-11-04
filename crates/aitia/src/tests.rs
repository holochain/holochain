use std::collections::HashSet;

use maplit::hashset;

use super::simple_report as report;
use crate::cause::*;
use crate::graph::*;

fn path_lengths<T: Fact>(graph: &CauseTree<T>, start: Cause<T>, end: Cause<T>) -> Vec<usize> {
    let start_ix = graph
        .node_indices()
        .find(|i| graph[*i].cause == start)
        .unwrap();
    let end_ix = graph
        .node_indices()
        .find(|i| graph[*i].cause == end)
        .unwrap();
    petgraph::algo::all_simple_paths::<Vec<_>, _>(&**graph, start_ix, end_ix, 0, None)
        .map(|c| c.len())
        .collect()
}

type Checks<T> = Box<dyn Fn(&T) -> bool>;

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
    
        fn cause(&self, (): &Self::Context) -> CauseResult<Self> {
            Ok(self.1.then_some(self.clone().into()))
        }
    }
    
    #[test_case( Singleton(false, false) => Some(hashset! { Singleton(false, false).into() }) ; "failing single isolated fact produces only self")]
    #[test_case( Singleton(false, true)  => Some(hashset! { Singleton(false, true).into() }) ; "failing single self-referencing fact produces only self")]
    #[test_case( Singleton(true, false) => None ; "passing single isolated fact produces Pass")]
    #[test_case( Singleton(true, true) => None  ; "passing single self-referencing fact produces Pass")]
    fn singleton_causes(fact: Singleton) -> Option<HashSet<Cause<Singleton>>> {
        holochain_trace::test_run().ok().unwrap();
        Cause::from(fact.clone())
            .traverse(&())
            .fail()
            .map(|(graph, _)| graph.causes())
    }
}

mod acyclic_single_path {
    use super::*;
    use test_case::test_case;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Countdown(u8);
    
    
    impl Fact for Countdown {
        type Context = Checks<Self>;
    
        fn cause(&self, _: &Self::Context) -> CauseResult<Self> {
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
        => vec![0, 1, 2, 3]
        ; "Countdown from 3 with all false returns path from 3 to 0"
    )]
    #[test_case( 
        Countdown(2), 
        Some(3) 
        => vec![0, 1, 2]
        ; "Countdown from 2 with 3 being true returns path from 2 to 0"
    )]
    #[test_case( 
        Countdown(3), 
        Some(0) 
        => vec![1, 2, 3]
        ; "Countdown from 3 with 0 being true returns path from 3 to 1"
    )]
    #[test_case( 
        Countdown(3), 
        Some(1) 
        => vec![2, 3]
        ; "Countdown from 3 with 1 being true returns path from 3 to 2"
    )]
    #[test_case( 
        Countdown(1), 
        Some(2) 
        => vec![0, 1]
        ; "Countdown from 1 with 3 being true returns path from 1 to 0"
    )]
    fn single_path(countdown: Countdown, true_one: Option<u8>) -> Vec<u8> {
        holochain_trace::test_run().ok().unwrap();
    
        let checker: Checks<Countdown> = Box::new(move |c| Some(c.0) == true_one);
        let tr = Cause::from(countdown).traverse(&checker);
        report(&tr);
        let (graph, _passes) = tr
            .fail()
            .unwrap();
    
        let mut nodes: Vec<_> = graph.causes().into_iter().map(|c| match c {
            Cause::Fact(f) => f.0,
            _ => unreachable!(),
        }).collect();
        nodes.sort();
        
        let edges = graph.edge_count();
    
        // If the number of edges is one less than the number of nodes, that implies a straight noncyclic path
        // (this doesn't test that something weird and ridiculous happens like a branch with a disconnected node)
        assert_eq!(nodes.len(), edges + 1);
        nodes
    }
}

mod single_loop {
    use super::*;
    use test_case::test_case;

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Countdown(u8);

    impl Fact for Countdown {
        type Context = Checks<Self>;

        fn cause(&self, _: &Self::Context) -> CauseResult<Self> {
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

    #[test]
    fn loopy() {
        holochain_trace::test_run().ok().unwrap();
    
    
        let true_0: Checks<Countdown> = Box::new(|i| i.0 == 0);
        let true_1: Checks<Countdown> = Box::new(|i| i.0 == 1);
        let true_3: Checks<Countdown> = Box::new(|i| i.0 == 3);
        {
            let tr = Cause::from(Countdown(3)).traverse(&true_0);
            report(&tr);
            let (graph, _passes) = tr.fail().unwrap();
            assert_eq!(
                graph.causes(),
                maplit::hashset![
                    Cause::from(Countdown(3)),
                    Cause::from(Countdown(2)),
                    Cause::from(Countdown(1)),
                ]
            );
            // The graph should show the loop
            assert_eq!(graph.edge_count(), 3);
        }
    
        {
            let tr = Cause::from(Countdown(3)).traverse(&true_1);
            let (graph, _passes) = tr.fail().unwrap();
            assert_eq!(
                graph.causes(),
                maplit::hashset![Cause::from(Countdown(3)), Cause::from(Countdown(2))]
            );
            assert_eq!(graph.edge_count(), 1);
        }
        {
            let tr = Cause::from(Countdown(2)).traverse(&true_3);
            let (graph, _passes) = tr.fail().unwrap();
            assert_eq!(
                graph.causes(),
                maplit::hashset![Cause::from(Countdown(2)), Cause::from(Countdown(1))]
            );
            assert_eq!(graph.edge_count(), 1);
        }
    }
    
}

#[test]
fn branching_any() {
    holochain_trace::test_run().ok().unwrap();

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Branching(u8);

    impl Fact for Branching {
        type Context = HashSet<u8>;

        fn cause(&self, _ctx: &Self::Context) -> CauseResult<Self> {
            Ok((self.0 <= 64)
                .then(|| Cause::any(vec![Self(self.0 * 2).into(), Self(self.0 * 2 + 1).into()])))
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            ctx.contains(&self.0)
        }
    }

    {
        let ctx = maplit::hashset![40, 64];
        let tr = Cause::from(Branching(2)).traverse(&ctx);
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
        let tr = Cause::from(Branching(2)).traverse(&ctx);
        // report(&tr);
        let _ = tr.fail().unwrap();
        // no assertion here, just a smoke test. It's a neat case.
    }
}

#[test]
fn simple_every() {
    holochain_trace::test_run().ok().unwrap();

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

        fn cause(&self, _ctx: &Self::Context) -> CauseResult<Self> {
            use Recipe::*;
            Ok(match self {
                Eggs => None,
                Vinegar => None,
                Mayo => Some(Cause::every(vec![Eggs.into(), Vinegar.into()])),
                Tuna => None,
                Cheese => None,
                Bread => None,
                TunaSalad => Some(Cause::every(vec![Tuna.into(), Mayo.into()])),
                TunaMelt => Some(Cause::every(vec![
                    TunaSalad.into(),
                    Cheese.into(),
                    Bread.into(),
                ])),
                GrilledCheese => Some(Cause::every(vec![Cheese.into(), Bread.into()])),
            })
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            ctx.contains(&self)
        }
    }

    {
        // TODO:
        // - loops with Every might take some more thought.

        {
            let ctx = maplit::hashset![Cheese, Bread];
            let tr = Cause::from(GrilledCheese).traverse(&ctx);
            report(&tr);
            let (g, _) = tr.fail().unwrap();
            assert_eq!(g.causes(), maplit::hashset!(Cause::from(GrilledCheese)));
        }
        {
            let ctx = maplit::hashset![Cheese, Bread];
            let tr = Cause::from(TunaMelt).traverse(&ctx);
            report(&tr);
            let (g, _) = tr.fail().unwrap();
            assert_eq!(
                g.causes()
                    .intersection(&hashset! {Tuna.into(), Vinegar.into(), Eggs.into()})
                    .count(),
                3
            );
        }
        {
            let ctx = maplit::hashset![Cheese, Bread, TunaSalad];
            let tr = Cause::from(TunaMelt).traverse(&ctx);
            report(&tr);
            let (g, _) = tr.fail().unwrap();
            assert_eq!(g.causes(), maplit::hashset!(Cause::from(TunaMelt)));
        }

        {
            let ctx = maplit::hashset![Cheese, Bread, Eggs, Tuna];
            let tr = Cause::from(TunaMelt).traverse(&ctx);
            report(&tr);
            let (g, _) = tr.fail().unwrap();

            // Only the Vinegar base ingredient is included
            assert_eq!(
                g.causes()
                    .intersection(&hashset! {Tuna.into(), Vinegar.into(), Eggs.into()})
                    .cloned()
                    .collect::<HashSet<_>>(),
                hashset! {Cause::from(Vinegar)}
            );
        }

        {
            let ctx = maplit::hashset![Cheese, Bread, Mayo];
            let tr = Cause::from(TunaMelt).traverse(&ctx);
            report(&tr);
            let (g, _) = tr.fail().unwrap();

            // Only the Tuna base ingredient is included
            assert_eq!(
                g.causes()
                    .intersection(&hashset! {Tuna.into(), Vinegar.into(), Eggs.into()})
                    .cloned()
                    .collect::<HashSet<_>>(),
                hashset! {Cause::from(Tuna)}
            );
        }
    }
}

#[test]
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

        fn cause(&self, _ctx: &Self::Context) -> CauseResult<Self> {
            use Stage::*;
            Ok(match self.stage {
                Create => None,
                Fetch => Some(Cause::any_named(
                    "Receive either",
                    vec![self.mine(ReceiveA), self.mine(ReceiveB)],
                )),
                ReceiveA => Some(self.theirs(SendA)),
                ReceiveB => Some(self.theirs(SendB)),
                Store => Some(Cause::any_named(
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
        pub fn mine(&self, stage: Stage) -> Cause<Self> {
            Cause::Fact(Self {
                which: self.which,
                stage,
            })
        }

        pub fn theirs(&self, stage: Stage) -> Cause<Self> {
            Cause::Fact(Self {
                which: !self.which,
                stage,
            })
        }
    }

    let fatma_store = Cause::Fact(F {
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
