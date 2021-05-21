//! Genesis Workflow: Initialize the source chain with the initial entries:
//! - Dna
//! - AgentValidationPkg
//! - AgentId
//!

// FIXME: understand the details of actually getting the DNA
// FIXME: creating entries in the config db

use super::error::WorkflowError;
use super::error::WorkflowResult;
use crate::core::{
    queue_consumer::OneshotWriter,
    ribosome::guest_callback::genesis_self_check::{
        GenesisSelfCheckHostAccess, GenesisSelfCheckInvocation, GenesisSelfCheckResult,
    },
};
use crate::{conductor::api::CellConductorApiT, core::ribosome::RibosomeT};
use derive_more::Constructor;
use holochain_lmdb::prelude::*;
use holochain_state::source_chain::SourceChainBuf;
use holochain_state::workspace::Workspace;
use holochain_state::workspace::WorkspaceResult;
use holochain_types::prelude::*;
use tracing::*;

/// The struct which implements the genesis Workflow
#[derive(Constructor, Debug)]
pub struct GenesisWorkflowArgs<Ribosome>
where
    Ribosome: RibosomeT + Send + 'static,
{
    dna_file: DnaFile,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<SerializedBytes>,
    ribosome: Ribosome,
}

#[instrument(skip(workspace, writer, api))]
pub async fn genesis_workflow<'env, Api: CellConductorApiT, Ribosome>(
    mut workspace: GenesisWorkspace,
    writer: OneshotWriter,
    api: Api,
    args: GenesisWorkflowArgs<Ribosome>,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + Send + 'static,
{
    genesis_workflow_inner(&mut workspace, args, api).await?;

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    // commit the workspace
    writer.with_writer(|writer| workspace.flush_to_txn(writer))?;

    Ok(())
}

async fn genesis_workflow_inner<Api: CellConductorApiT, Ribosome>(
    workspace: &mut GenesisWorkspace,
    args: GenesisWorkflowArgs<Ribosome>,
    api: Api,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + Send + 'static,
{
    let GenesisWorkflowArgs {
        dna_file,
        agent_pubkey,
        membrane_proof,
        ribosome,
    } = args;

    if workspace.source_chain.has_genesis() {
        return Ok(());
    }

    let result = ribosome.run_genesis_self_check(
        GenesisSelfCheckHostAccess,
        GenesisSelfCheckInvocation {
            payload: GenesisSelfCheckData {
                dna_def: dna_file.dna_def().clone(),
                membrane_proof: membrane_proof.clone(),
                agent_key: agent_pubkey.clone(),
            },
        },
    )?;

    // If the self-check fails, fail genesis, and don't create the source chain.
    if let GenesisSelfCheckResult::Invalid(reason) = result {
        return Err(WorkflowError::GenesisFailure(reason));
    }

    // TODO: this is a placeholder for a real DPKI request to show intent
    if api
        .dpki_request("is_agent_pubkey_valid".into(), agent_pubkey.to_string())
        .await
        .expect("TODO: actually implement this")
        == "INVALID"
    {
        return Err(WorkflowError::AgentInvalid(agent_pubkey.clone()));
    }

    workspace
        .source_chain
        .genesis(
            dna_file.dna_hash().clone(),
            agent_pubkey.clone(),
            membrane_proof,
        )
        .await
        .map_err(WorkflowError::from)?;

    Ok(())
}

/// The workspace for Genesis
pub struct GenesisWorkspace {
    source_chain: SourceChainBuf,
}

impl GenesisWorkspace {
    /// Constructor
    pub async fn new(env: EnvironmentRead) -> WorkspaceResult<Self> {
        Ok(Self {
            source_chain: SourceChainBuf::new(env)?,
        })
    }
}

impl Workspace for GenesisWorkspace {
    fn flush_to_txn_ref(&mut self, writer: &mut Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn_ref(writer)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use crate::conductor::api::MockCellConductorApi;
    use crate::core::ribosome::MockRibosomeT;
    use crate::core::SourceChainResult;
    use fallible_iterator::FallibleIterator;
    use holochain_lmdb::test_utils::test_cell_env;
    use holochain_state::source_chain::SourceChain;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_file;
    use holochain_zome_types::Header;
    use matches::assert_matches;
    use observability;

    pub async fn fake_genesis(source_chain: &mut SourceChain) -> SourceChainResult<()> {
        let dna = fake_dna_file("cool dna");
        let dna_hash = dna.dna_hash().clone();
        let agent_pubkey = fake_agent_pubkey_1();

        source_chain.genesis(dna_hash, agent_pubkey, None).await
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn genesis_initializes_source_chain() -> Result<(), anyhow::Error> {
        observability::test_run()?;
        let test_env = test_cell_env();
        let arc = test_env.env();
        let dna = fake_dna_file("a");
        let agent_pubkey = fake_agent_pubkey_1();

        {
            let workspace = GenesisWorkspace::new(arc.clone().into()).await?;
            let mut api = MockCellConductorApi::new();
            api.expect_sync_dpki_request()
                .returning(|_, _| Ok("mocked dpki request response".to_string()));
            let mut ribosome = MockRibosomeT::new();
            ribosome
                .expect_run_genesis_self_check()
                .returning(|_, _| Ok(GenesisSelfCheckResult::Valid));
            let args = GenesisWorkflowArgs {
                dna_file: dna.clone(),
                agent_pubkey: agent_pubkey.clone(),
                membrane_proof: None,
                ribosome,
            };
            let _: () = genesis_workflow(workspace, arc.clone().into(), api, args).await?;
        }

        {
            let source_chain = SourceChain::new(arc.clone().into())?;
            assert_eq!(source_chain.agent_pubkey()?, agent_pubkey);
            source_chain.chain_head().expect("chain head should be set");

            let mut iter = source_chain.iter_back();
            let mut headers = Vec::new();

            while let Some(h) = iter.next().unwrap() {
                let (h, _) = h.into_header_and_signature();
                let (h, _) = h.into_inner();
                headers.push(h);
            }

            assert_matches!(
                headers.as_slice(),
                [
                    Header::Create(_),
                    Header::AgentValidationPkg(_),
                    Header::Dna(_)
                ]
            );
        }

        Ok(())
    }
}

/* TODO: make doc-able

Called from:

 - Conductor upon first ACTIVATION of an installed DNA (trace: follow)



Parameters (expected types/structures):

- DNA hash to pull from path to file (or HCHC [FUTURE] )

- AgentID [SEEDLING] (already registered in DeepKey [LEAPFROG])

- Membrane Access Payload (optional invitation code / to validate agent join) [possible for LEAPFROG]



Data X (data & structure) from Store Y:

- Get DNA from HCHC by DNA hash

- or Get DNA from filesystem by filename



----

Functions / Workflows:

- check that agent key is valid [MOCKED dpki] (via real dpki [LEAPFROG])

- retrieve DNA from file path [in the future from HCHC]

- initialize lmdb environment and dbs, save to conductor runtime config.

- commit DNA entry (w/ special enum header with NULL  prev_header)

- commit CapGrant for author (agent key) (w/ normal header)



    fn commit_DNA

    fn produce_header



Examples / Tests / Acceptance Criteria:

- check hash of DNA =



----



Persisted X Changes to Store Y (data & structure):

- source chain HEAD 2 new headers

- CAS commit headers and genesis entries: DNA & Author Capabilities Grant (Agent Key)



- bootstrapped peers from attempt to publish key and join network



Spawned Tasks (don't wait for result -signals/log/tracing=follow):

- ZomeCall:init (for processing app initialization with bridges & networking)

- DHT transforms of genesis entries in CAS



Returned Results (type & structure):

- None
*/
