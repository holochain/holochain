use std::collections::HashSet;

use petgraph::visit::IntoEdges;

use crate::cause::*;
use crate::graph::*;

type Checks<T> = Box<dyn Fn(&T) -> bool>;

fn report<T: Fact>(traversal: &Traversal<T>) {
    match traversal {
        Traversal::Pass => println!("PASS"),
        Traversal::Groundless => println!("GROUNDLESS"),
        Traversal::Fail(tree) => {
            let dot = format!(
                "{:?}",
                petgraph::dot::Dot::with_config(&tree, &[petgraph::dot::Config::EdgeNoLabel],)
            );

            if let Ok(graph) = graph_easy(&dot) {
                println!("`graph-easy` output:\n{}", graph);
            } else {
                println!("`graph-easy` not installed. Original dot output: {}", dot);
            }
        }
    }
}

fn path_lengths<T: Fact>(graph: &TruthTree<T>, start: Cause<T>, end: Cause<T>) -> Vec<usize> {
    let start_ix = graph.node_indices().find(|i| graph[*i] == start).unwrap();
    let end_ix = graph.node_indices().find(|i| graph[*i] == end).unwrap();
    petgraph::algo::all_simple_paths::<Vec<_>, _>(graph, start_ix, end_ix, 0, None)
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

        fn cause(&self, (self_ref, _): &Self::Context) -> Option<Cause<Self>> {
            self_ref.then_some(Self.into())
        }

        fn check(&self, (_, check): &Self::Context) -> bool {
            *check
        }
    }

    // No basis in truth
    assert!(matches!(
        Cause::from(Singleton).traverse(&(false, false)),
        Traversal::Groundless
    ));
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
    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Countdown(u8);

    impl Fact for Countdown {
        type Context = Checks<Self>;

        fn cause(&self, _: &Self::Context) -> Option<Cause<Self>> {
            match self.0 {
                3 => Some(Self(2).into()),
                2 => Some(Self(1).into()),
                1 => Some(Self(0).into()),
                0 => None,
                _ => unreachable!(),
            }
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
        let tr = Cause::from(Countdown(3)).traverse(&all_false);
        assert!(matches!(tr, Traversal::Groundless));
    }
    {
        let tr = Cause::from(Countdown(2)).traverse(&true_3);
        assert!(matches!(tr, Traversal::Groundless));
    }
    {
        let graph = Cause::from(Countdown(3)).traverse(&true_0).fail().unwrap();
        let nodes = graph.node_weights().cloned().collect::<HashSet<_>>();

        assert_eq!(
            nodes,
            maplit::hashset![
                Cause::from(Countdown(1)),
                Cause::from(Countdown(2)),
                Cause::from(Countdown(3))
            ]
        );

        assert_eq!(graph.edge_count(), 2);
    }
    {
        let graph = Cause::from(Countdown(3)).traverse(&true_1).fail().unwrap();
        let nodes = graph.node_weights().cloned().collect::<HashSet<_>>();

        assert_eq!(
            nodes,
            maplit::hashset![Cause::from(Countdown(2)), Cause::from(Countdown(3))]
        );

        assert_eq!(graph.edge_count(), 1);
    }
}

#[test]
fn branching_any() {
    #[derive(Clone, Debug, PartialEq, Eq, Hash, derive_more::Display)]
    struct Branching(u8);

    impl Fact for Branching {
        type Context = HashSet<u8>;

        fn cause(&self, _ctx: &Self::Context) -> Option<Cause<Self>> {
            (self.0 <= 64)
                .then(|| Cause::Any(vec![Self(self.0 * 2).into(), Self(self.0 * 2 + 1).into()]))
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            ctx.contains(&self.0)
        }
    }

    {
        let tr = Cause::from(Branching(2)).traverse(&maplit::hashset![40, 64]);
        report(&tr);
        let graph = tr.fail().unwrap();
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
        let _graph = tr.fail().unwrap();
        // no assertion here, just a smoke test. It's a neat case.
    }
}

#[test]
fn holochain_like() {
    holochain_trace::test_run().ok().unwrap();

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, derive_more::Constructor)]
    struct Step {
        which: bool,
        stage: Stage,
    }

    impl std::fmt::Display for Step {
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

    impl Fact for Step {
        type Context = Checks<Self>;

        fn cause(&self, _ctx: &Self::Context) -> Option<Cause<Self>> {
            use Stage::*;
            match self.stage {
                Create => None,
                Fetch => Some(Cause::Any(vec![self.mine(ReceiveA), self.mine(ReceiveB)])),
                ReceiveA => Some(self.theirs(SendA)),
                ReceiveB => Some(self.theirs(SendB)),
                Store => Some(Cause::Any(vec![self.mine(Create), self.mine(Fetch)])),
                SendA => Some(self.mine(Store)),
                SendB => Some(self.mine(Store)),
            }
        }

        fn check(&self, ctx: &Self::Context) -> bool {
            (ctx)(self)
        }

        fn explain(&self, _ctx: &Self::Context) -> String {
            self.to_string()
        }
    }

    impl Step {
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

    let fatma_store = Cause::Fact(Step {
        which: false,
        stage: Stage::Store,
    });

    let checks: Checks<Step> = Box::new(|step: &Step| match (step.which, step.stage) {
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
