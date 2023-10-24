use crate::cause::*;
use crate::graph::*;

type Checks<T> = Box<dyn Fn(&T) -> bool>;

fn report<T: Fact>(tree: &Tree<T>) {
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
    assert_eq!(
        Cause::from(Singleton).graph(&(false, false)).node_count(),
        0
    );
    // Loop ending in falsity
    assert_eq!(Cause::from(Singleton).graph(&(true, false)).node_count(), 0);
    // Self is true
    assert_eq!(Cause::from(Singleton).graph(&(false, true)).node_count(), 1);
    // Self is true and loopy
    assert_eq!(Cause::from(Singleton).graph(&(true, true)).node_count(), 1);
}

#[test]
fn no_truth() {
    let all_false: Checks<Countdown> = Box::new(|_| false);
    let true_0: Checks<Countdown> = Box::new(|i| i.0 == 0);

    let tree = Cause::from(Countdown(3)).graph(&all_false);
    assert_eq!(tree.node_count(), 0);

    let tree = Cause::from(Countdown(3)).graph(&true_0);
    report(&tree);
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

    let table = traverse(&fatma_store, &checks);
    let sub = prune_traversal(&table, &fatma_store);

    println!("TABLE\n{:#?}", table);
    println!("SUBTABLE\n{:#?}", sub);

    let g = produce_graph(&table, &fatma_store);

    let dot = format!(
        "{:?}",
        petgraph::dot::Dot::with_config(&g, &[petgraph::dot::Config::EdgeNoLabel],)
    );

    if let Ok(graph) = graph_easy(&dot) {
        println!("`graph-easy` output:\n{}", graph);
    } else {
        println!("`graph-easy` not installed. Original dot output: {}", dot);
    }
}
