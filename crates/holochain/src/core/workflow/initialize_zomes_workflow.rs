use super::{
    error::WorkflowResult, unsafe_invoke_zome_workspace::UnsafeInvokeZomeWorkspace,
    InvokeZomeWorkspace, Workflow, WorkflowEffects,
};
use crate::core::{
    ribosome::{
        error::RibosomeResult,
        guest_callback::init::{InitInvocation, InitResult},
        wasm_ribosome::WasmRibosome,
        RibosomeT,
    },
    state::workspace::{Workspace, WorkspaceError},
};
use futures::FutureExt;
use holo_hash::AgentPubKey;
use holochain_state::buffer::BufferedStore;
use holochain_state::prelude::Writer;
use holochain_types::{dna::DnaFile, header::InitZomesComplete, Header, Timestamp};
use must_future::MustBoxFuture;

pub(crate) struct InitializeZomesWorkflow {
    pub dna_file: DnaFile,
    pub agent_key: AgentPubKey,
}

impl<'env> Workflow<'env> for InitializeZomesWorkflow {
    type Output = RibosomeResult<InitResult>;
    type Workspace = InitializeZomesWorkspace<'env>;
    type Triggers = ();

    fn workflow(
        self,
        mut workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self>> {
        async {
            let Self {
                dna_file,
                agent_key: author,
            } = self;
            // Get the ribosome
            let ribosome = WasmRibosome::new(dna_file.clone());

            // Call the init callback
            let result = {
                // TODO: We need a better solution then reusung the InvokeZomeWorkspace (i.e. ghost actor)
                let (_g, raw_workspace) = UnsafeInvokeZomeWorkspace::from_mut(&mut workspace.0);
                let invocation = InitInvocation {
                    dna_def: dna_file.dna().clone(),
                };
                ribosome.run_init(raw_workspace, invocation)
            };

            // Insert the init marker
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
