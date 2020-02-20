use crate::HolochainState;

pub trait Workflow {
    pub fn start(self) -> Future<()>;
}

struct CellContext {
  conductor_api: ConductorApiRef  // cheaply clonable refernce to something that allows us to send data to conductor from anywhere
  // stuff goes here.
}

pub fn finish_workflow(result: WorkflowResult) {
  info!("finishing workflow {}", ((result.name)));

  finalize_workspace(result.workspace)
  // actua
}


struct WorkflowResult {
  workspace: Workspace,
  triggers: Vec<WorkflowTrigger>,
}

enum Trigger {
  Workflow,
  Callback,
  Signal,
}

enum TriggerDelay {
  Immediate
}

struct Workspace;
