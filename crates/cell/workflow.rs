use crate::HolochainState;

pub trait Workflow {
    pub fn start(self) -> Future<()>;
}

struct CellContext {
  conductor_api: ConductorApiRef  // cheaply clonable refernce to something that allows us to send data to conductor from anywhere
  // stuff goes here.
}
