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
use holochain_state::buffer::BufferedStore;
use holochain_state::prelude::Writer;
use holochain_types::{dna::DnaDef, header::HeaderBuilder};
use must_future::MustBoxFuture;

pub(crate) struct InitializeZomesWorkflow<Ribosome: RibosomeT> {
    pub dna_def: DnaDef,
    pub ribosome: Ribosome,
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
            let Self { dna_def, ribosome } = self;
            // Call the init callback
            let result = {
                // TODO: We need a better solution then reusung the InvokeZomeWorkspace (i.e. ghost actor)
                let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace.0);
                let invocation = InitInvocation { dna_def };
                ribosome.run_init(raw_workspace, invocation)
            };

            // Insert the init marker
            workspace
                .0
                .source_chain
                .put(HeaderBuilder::InitZomesComplete, None)
                .await?;

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

pub(crate) struct InitializeZomesWorkspace<'env>(pub(crate) InvokeZomeWorkspace<'env>);

impl<'env> Workspace<'env> for InitializeZomesWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> Result<(), WorkspaceError> {
        self.0.source_chain.into_inner().flush_to_txn(&mut writer)?;
        self.0.meta.flush_to_txn(&mut writer)?;
        self.0.cache_cas.flush_to_txn(&mut writer)?;
        self.0.cache_meta.flush_to_txn(&mut writer)?;
        Ok(writer.commit()?)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::workflow::fake_genesis;
    use crate::fixt::DnaDefFixturator;
    use fixt::Unpredictable;
    use holochain_state::{env::ReadManager, test_utils::test_cell_env};
    use holochain_types::Header;
    use matches::assert_matches;

    #[tokio::test(threaded_scheduler)]
    async fn adds_init_marker() {
        let env = test_cell_env();
        let dbs = env.dbs().await;
        let env_ref = env.guard().await;
        let reader = env_ref.reader().unwrap();
        let mut workspace =
            InitializeZomesWorkspace(InvokeZomeWorkspace::new(&reader, &dbs).unwrap());
        let mut ribosome = MockRibosomeT::new();

        // Setup the ribosome mock
        ribosome
            .expect_run_init()
            .returning(move |_workspace, _invocation| Ok(InitResult::Pass));

        // Genesis
        fake_genesis(&mut workspace.0.source_chain).await.unwrap();

        let dna_def = DnaDefFixturator::new(Unpredictable).next().unwrap();

        let workflow = InitializeZomesWorkflow { ribosome, dna_def };
        let (_, effects) = workflow.workflow(workspace).await.unwrap();

        // Check the initialize zome was added to a trigger
        assert!(effects.signals.is_empty());
        assert!(effects.callbacks.is_empty());

        // Check init is added to the workspace
        assert_matches!(
            effects.workspace.0.source_chain.get_index(3).await,
            Ok(Some(Header::InitZomesComplete(_)))
        );
    }
}
