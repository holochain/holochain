
use super::{error::WorkflowResult, Workflow};
use crate::core::state::workspace::{Workspace, WorkspaceError};
use holochain_state::prelude::Writer;
use must_future::MustBoxFuture;
use futures::FutureExt;

#[derive(Debug)]
pub(crate) struct InitializeZomesWorkflow { }


impl<'env> Workflow<'env> for InitializeZomesWorkflow
{
    type Output = ();
    type Workspace = InitializeZomesWorkspace;
    type Triggers = ();

    fn workflow(
        self,
        workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self::Output, Self>> {
        async {
            unimplemented!()
        }.boxed().into()
    }
}


pub(crate) struct InitializeZomesWorkspace;

impl<'env> Workspace<'env> for InitializeZomesWorkspace {
    fn commit_txn(self, writer: Writer) -> Result<(), WorkspaceError> {
        unimplemented!()
    }
}