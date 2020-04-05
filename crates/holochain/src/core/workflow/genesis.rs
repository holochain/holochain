use super::{WorkflowEffects, WorkflowError, WorkflowResult};
use crate::{conductor::api::CellConductorApi, core::state::workspace::GenesisWorkspace};
use sx_types::{agent::AgentId, dna::Dna, entry::Entry};

/// Initialize the source chain with the initial entries:
/// - Dna
/// - AgentId
/// - CapTokenGrant
///
/// FIXME: understand the details of actually getting the DNA
/// FIXME: creating entries in the config db
pub async fn genesis(
    mut workspace: GenesisWorkspace<'_>,
    api: impl CellConductorApi,
    dna: Dna,
    agent_id: AgentId,
) -> WorkflowResult<GenesisWorkspace<'_>> {
    if api
        .dpki_request("is_agent_id_valid".into(), agent_id.pub_sign_key().into())
        .await?
        == "INVALID"
    {
        return Err(WorkflowError::AgentIdInvalid(agent_id.clone()));
    }

    workspace
        .source_chain
        .put_entry(Entry::Dna(Box::new(dna)), &agent_id);
    workspace
        .source_chain
        .put_entry(Entry::AgentId(agent_id.clone()), &agent_id);

    Ok(WorkflowEffects {
        workspace,
        triggers: Default::default(),
        signals: Default::default(),
        callbacks: Default::default(),
    })
}

#[cfg(test)]
mod tests {

    use super::genesis;
    use crate::{
        conductor::api::MockCellConductorApi,
        core::{
            state::{
                source_chain::SourceChain,
                workspace::{GenesisWorkspace, Workspace},
            },
            workflow::WorkflowError,
        },
    };
    use fallible_iterator::FallibleIterator;
    use sx_state::{env::*, test_utils::test_cell_env};
    use sx_types::{
        entry::Entry,
        observability,
        prelude::*,
        test_utils::{fake_agent_id, fake_dna},
    };
    use tracing::*;

    #[tokio::test]
    async fn genesis_initializes_source_chain() -> Result<(), anyhow::Error> {
        observability::test_run()?;
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        let dna = fake_dna("a");
        let agent_id = fake_agent_id("a");

        {
            let reader = env.reader()?;
            let workspace = GenesisWorkspace::new(&reader, &dbs)?;
            let mut api = MockCellConductorApi::new();
            api.expect_sync_dpki_request()
                .returning(|_, _| Ok("mocked dpki request response".to_string()));
            let fx = genesis(workspace, api, dna.clone(), agent_id.clone()).await?;
            let writer = env.writer()?;
            fx.workspace.commit_txn(writer)?;
        }

        env.with_reader(|reader| {
            let source_chain = SourceChain::new(&reader, &dbs)?;
            assert_eq!(source_chain.agent_id()?, agent_id);
            source_chain.chain_head().expect("chain head should be set");
            let addresses: Vec<_> = source_chain
                .iter_back()
                .map(|h| {
                    debug!("header: {:?}", h);
                    Ok(h.entry_address().clone())
                })
                .collect()
                .unwrap();
            assert_eq!(
                addresses,
                vec![
                    Entry::AgentId(agent_id).address(),
                    Entry::Dna(Box::new(dna)).address()
                ]
            );
            Result::<_, WorkflowError>::Ok(())
        })?;
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

- commit CapToken Grant for author (agent key) (w/ normal header)



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
