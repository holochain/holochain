use std::collections::HashSet;

use maplit::hashset;

use crate::cause::*;
use crate::graph::*;

type Checks<T> = Box<dyn Fn(&T) -> bool>;

fn report<T: Fact>(traversal: &Traversal<T>) {
    match traversal {
        Traversal::Pass => println!("PASS"),
        Traversal::Groundless => println!("GROUNDLESS"),
        Traversal::Fail { tree, passes } => {
            let dot = format!(
                "{:?}",
                petgraph::dot::Dot::with_config(&**tree, &[petgraph::dot::Config::EdgeNoLabel],)
            );

            if let Ok(graph) = graph_easy(&dot) {
                println!("`graph-easy` output:\n{}", graph);
            } else {
                println!("`graph-easy` not installed. Original dot output: {}", dot);
            }

            println!("Passing checks: {:#?}", passes);
        }
        Traversal::TraversalError(err) => println!("Traversal error: {:?}", err),
    }
}

fn path_lengths<T: Fact>(graph: &TruthTree<T>, start: Cause<T>, end: Cause<T>) -> Vec<usize> {
    let start_ix = graph.node_indices().find(|i| graph[*i] == start).unwrap();
    let end_ix = graph.node_indices().find(|i| graph[*i] == end).unwrap();
    petgraph::algo::all_simple_paths::<Vec<_>, _>(&**graph, start_ix, end_ix, 0, None)
        .map(|c| c.len())
        .collect()
}

#[test]
fn singleton() {
    holochain_trace::test_run().ok().unwrap();

    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Singleton;

    impl Fact for Singleton {
        type Context = (bool, bool);

        fn cause(&self, (self_ref, _): &Self::Context) -> CauseResult<Self> {
            Ok(self_ref.then_some(Self.into()))
        }

        fn check(&self, (_, check): &Self::Context) -> bool {
            *check
        }
    }

    // No basis in truth
    let (graph, _passes) = Cause::from(Singleton)
        .traverse(&(false, false))
        .fail()
        .unwrap();
    assert_eq!(graph.nodes(), maplit::hashset! {Singleton.into()});
    assert_eq!(graph.edge_count(), 0);

    // Loop ending in falsity
    assert!(matches!(
        Cause::from(Singleton).traverse(&(true, false)),
        Traversal::Groundless
    ));

    // Self is true
    assert!(matches!(
        Cause::from(Singleton).traverse(&(false, true)),
        Traversal::Pass
    ));

    // Self is true and loopy
    assert!(matches!(
        Cause::from(Singleton).traverse(&(true, true)),
        Traversal::Pass
    ));
}

#[test]
fn single_path() {
    holochain_trace::test_run().ok().unwrap();

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

    let all_false: Checks<Countdown> = Box::new(|_| false);
    let true_0: Checks<Countdown> = Box::new(|i| i.0 == 0);
    let true_1: Checks<Countdown> = Box::new(|i| i.0 == 1);
    let true_3: Checks<Countdown> = Box::new(|i| i.0 == 3);
    {
        let (graph, _passes) = Cause::from(Countdown(3))
            .traverse(&all_false)
            .fail()
            .unwrap();
        assert_eq!(
            graph.nodes(),
            maplit::hashset![
                Cause::from(Countdown(0)),
                Cause::from(Countdown(1)),
                Cause::from(Countdown(2)),
                Cause::from(Countdown(3)),
            ]
        );
        assert_eq!(graph.edge_count(), 3);
    }
    {
        let (graph, _passes) = Cause::from(Countdown(2)).traverse(&true_3).fail().unwrap();
        assert_eq!(
            graph.nodes(),
            maplit::hashset![
                Cause::from(Countdown(0)),
                Cause::from(Countdown(1)),
                Cause::from(Countdown(2)),
            ]
        );
        assert_eq!(graph.edge_count(), 2);
    }
    {
        let (graph, _passes) = Cause::from(Countdown(3)).traverse(&true_0).fail().unwrap();
        assert_eq!(
            graph.nodes(),
            maplit::hashset![
                Cause::from(Countdown(1)),
                Cause::from(Countdown(2)),
                Cause::from(Countdown(3))
            ]
        );

        assert_eq!(graph.edge_count(), 2);
    }
    {
        let (graph, _passes) = Cause::from(Countdown(3)).traverse(&true_1).fail().unwrap();
        let nodes = graph.node_weights().cloned().collect::<HashSet<_>>();

        assert_eq!(
            nodes,
            maplit::hashset![Cause::from(Countdown(2)), Cause::from(Countdown(3))]
        );

        assert_eq!(graph.edge_count(), 1);
    }
}

#[test]
fn loopy() {
    holochain_trace::test_run().ok().unwrap();

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

    let true_0: Checks<Countdown> = Box::new(|i| i.0 == 0);
    let true_1: Checks<Countdown> = Box::new(|i| i.0 == 1);
    let true_3: Checks<Countdown> = Box::new(|i| i.0 == 3);

    assert!(matches!(
        Cause::from(Countdown(3)).traverse(&true_0),
        Traversal::Groundless
    ));

    {
        let tr = Cause::from(Countdown(3)).traverse(&true_1);
        let (graph, _passes) = tr.fail().unwrap();
        assert_eq!(
            graph.nodes(),
            maplit::hashset![Cause::from(Countdown(3)), Cause::from(Countdown(2))]
        );
        assert_eq!(graph.edge_count(), 1);
    }
    {
        let tr = Cause::from(Countdown(2)).traverse(&true_3);
        let (graph, _passes) = tr.fail().unwrap();
        assert_eq!(
            graph.nodes(),
            maplit::hashset![Cause::from(Countdown(2)), Cause::from(Countdown(1))]
        );
        assert_eq!(graph.edge_count(), 1);
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
                .then(|| Cause::Any(vec![Self(self.0 * 2).into(), Self(self.0 * 2 + 1).into()])))
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            ctx.contains(&self.0)
        }
    }

    {
        let tr = Cause::from(Branching(2)).traverse(&maplit::hashset![40, 64]);
        report(&tr);
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
        let tr = Cause::from(Branching(2)).traverse(&(32..128).collect());
        report(&tr);
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
                Mayo => Some(Cause::Every(vec![Eggs.into(), Vinegar.into()])),
                Tuna => None,
                Cheese => None,
                Bread => None,
                TunaSalad => Some(Cause::Every(vec![Tuna.into(), Mayo.into()])),
                TunaMelt => Some(Cause::Every(vec![
                    TunaSalad.into(),
                    Cheese.into(),
                    Bread.into(),
                ])),
                GrilledCheese => Some(Cause::Every(vec![Cheese.into(), Bread.into()])),
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
            let tr = Cause::from(GrilledCheese).traverse(&maplit::hashset![Cheese, Bread]);
            report(&tr);
            let (g, _) = tr.fail().unwrap();
            assert_eq!(g.nodes(), maplit::hashset!(Cause::from(GrilledCheese)));
        }
        {
            let tr = Cause::from(TunaMelt).traverse(&maplit::hashset![Cheese, Bread]);
            report(&tr);
            let (g, _) = tr.fail().unwrap();
            assert_eq!(
                g.nodes()
                    .intersection(&hashset! {Tuna.into(), Vinegar.into(), Eggs.into()})
                    .count(),
                3
            );
        }
        {
            let tr = Cause::from(TunaMelt).traverse(&maplit::hashset![Cheese, Bread, TunaSalad]);
            report(&tr);
            let (g, _) = tr.fail().unwrap();
            assert_eq!(g.nodes(), maplit::hashset!(Cause::from(TunaMelt)));
        }

        {
            let tr = Cause::from(TunaMelt).traverse(&maplit::hashset![Cheese, Bread, Eggs, Tuna]);
            report(&tr);
            let (g, _) = tr.fail().unwrap();

            // Only the Vinegar base ingredient is included
            assert_eq!(
                g.nodes()
                    .intersection(&hashset! {Tuna.into(), Vinegar.into(), Eggs.into()})
                    .cloned()
                    .collect::<HashSet<_>>(),
                hashset! {Cause::from(Vinegar)}
            );
        }

        {
            let tr = Cause::from(TunaMelt).traverse(&maplit::hashset![Cheese, Bread, Mayo]);
            report(&tr);
            let (g, _) = tr.fail().unwrap();

            // Only the Tuna base ingredient is included
            assert_eq!(
                g.nodes()
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
                Fetch => Some(Cause::Any(vec![self.mine(ReceiveA), self.mine(ReceiveB)])),
                ReceiveA => Some(self.theirs(SendA)),
                ReceiveB => Some(self.theirs(SendB)),
                Store => Some(Cause::Any(vec![self.mine(Create), self.mine(Fetch)])),
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
