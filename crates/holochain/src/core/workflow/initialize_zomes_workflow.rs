use super::{
    error::WorkflowResult, unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace,
    InvokeZomeWorkspace, Workflow, WorkflowEffects,
};
use crate::core::{
    ribosome::{
        error::RibosomeResult,
        guest_callback::init::{InitInvocation, InitResult},
        RibosomeT,
    },
    state::workspace::{Workspace, WorkspaceError},
};
use futures::FutureExt;
use holo_hash::AgentPubKey;
use holochain_state::prelude::Writer;
use holochain_types::{dna::DnaDef, header::InitZomesComplete, Header, Timestamp};
use must_future::MustBoxFuture;

pub(crate) struct InitializeZomesWorkflow<Ribosome: RibosomeT> {
    pub ribosome: Ribosome,
    pub dna_def: DnaDef,
    pub agent_key: AgentPubKey,
}

impl<'env, Ribosome> Workflow<'env> for InitializeZomesWorkflow<Ribosome>
where
    Ribosome: RibosomeT + Send + Sync + 'env,
{
    type Output = RibosomeResult<InitResult>;
    type Workspace = InitializeZomesWorkspace<'env>;
    type Triggers = ();

    fn workflow(
        self,
        mut workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self>> {
        async {
            let Self {
                ribosome,
                dna_def,
                agent_key: author,
            } = self;
            let result = {
                // TODO: We need a better solution then reusung the InvokeZomeWorkspace (i.e. ghost actor)
                let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace.0);
                let invocation = InitInvocation {
                    workspace: raw_workspace,
                    dna_def,
                };
                ribosome.run_init(invocation)
            };

            let prev_header = workspace.0.source_chain.chain_head()?;
            let init_header = Header::InitZomesComplete(InitZomesComplete {
                author,
                timestamp: Timestamp::now(),
                header_seq: 3,
                prev_header: prev_header.clone(),
            });
            workspace.0.source_chain.put(init_header, None).await?;

            let fx = WorkflowEffects {
                workspace,
                callbacks: Default::default(),
                signals: Default::default(),
                triggers: Default::default(),
            };

            Ok((result, fx))
        }
        .boxed()
        .into()
    }
}

pub(crate) struct InitializeZomesWorkspace<'env>(InvokeZomeWorkspace<'env>);

impl<'env> Workspace<'env> for InitializeZomesWorkspace<'env> {
    fn commit_txn(self, writer: Writer) -> Result<(), WorkspaceError> {
        Ok(writer.commit()?)
    }
}
