use crate::cause::*;
use crate::graph::*;

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

type StepValues = Box<dyn Fn(&Step) -> bool>;

impl Fact for Step {
    type Context = StepValues;

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

#[test]
fn one() {
    holochain_trace::test_run().ok().unwrap();

    let fatma_store = Cause::Fact(Step {
        which: false,
        stage: Stage::Store,
    });

    let checks: StepValues = Box::new(|step: &Step| match (step.which, step.stage) {
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
