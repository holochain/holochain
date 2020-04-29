use super::{WorkflowEffects, WorkflowError, WorkflowResult};
use crate::{conductor::api::CellConductorApiT, core::state::workspace::GenesisWorkspace};
use holochain_types::{chain_header::ChainHeader, dna::Dna, entry::Entry, header, prelude::*};

/// Initialize the source chain with the initial entries:
/// - Dna
/// - AgentId
/// - CapTokenGrant
///
/// FIXME: understand the details of actually getting the DNA
/// FIXME: creating entries in the config db
pub async fn genesis(
    mut workspace: GenesisWorkspace<'_>,
    api: impl CellConductorApiT,
    dna: Dna,
    agent_pubkey: AgentPubKey,
) -> WorkflowResult<GenesisWorkspace<'_>> {
    // TODO: this is a placeholder for a real DPKI request to show intent
    if api
        .dpki_request("is_agent_pubkey_valid".into(), agent_pubkey.to_string())
        .await?
        == "INVALID"
    {
        return Err(WorkflowError::AgentInvalid(agent_pubkey.clone()));
    }

    // create a DNA chain element and add it directly to the store
    let dna_header = ChainHeader::Dna(header::Dna {
        timestamp: chrono::Utc::now().timestamp().into(),
        author: agent_pubkey.clone(),
        hash: dna.dna_hash(),
    });
    workspace.source_chain.put(dna_header.clone(), None)?;

    // create a agent chain element and add it directly to the store
    let agent_header = ChainHeader::EntryCreate(header::EntryCreate {
        timestamp: chrono::Utc::now().timestamp().into(),
        author: agent_pubkey.clone(),
        prev_header: dna_header.hash().into(),
        entry_type: header::EntryType::AgentPubKey,
        entry_address: agent_pubkey.clone().into(),
    });
    workspace
        .source_chain
        .put(agent_header, Some(Entry::Agent(agent_pubkey)))?;

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
    use holochain_state::{env::*, test_utils::test_cell_env};
    use holochain_types::{
        chain_header::ChainHeader,
        header, observability,
        test_utils::{fake_agent_pubkey, fake_dna},
    };

    #[tokio::test]
    async fn genesis_initializes_source_chain() -> Result<(), anyhow::Error> {
        observability::test_run()?;
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await?;
        let dna = fake_dna("a");
        let agent_pubkey = fake_agent_pubkey("a");

        {
            let reader = env.reader()?;
            let workspace = GenesisWorkspace::new(&reader, &dbs)?;
            let mut api = MockCellConductorApi::new();
            api.expect_sync_dpki_request()
                .returning(|_, _| Ok("mocked dpki request response".to_string()));
            let fx = genesis(workspace, api, dna.clone(), agent_pubkey.clone()).await?;
            let writer = env.writer()?;
            fx.workspace.commit_txn(writer)?;
        }

        env.with_reader(|reader| {
            let source_chain = SourceChain::new(&reader, &dbs)?;
            assert_eq!(source_chain.agent_pubkey()?, agent_pubkey);
            source_chain.chain_head().expect("chain head should be set");
            let hashes: Vec<_> = source_chain
                .iter_back()
                .map(|h| {
                    Ok(match h.header() {
                        ChainHeader::Dna(header::Dna { .. }) => "Dna",
                        ChainHeader::LinkAdd(header::LinkAdd { .. }) => "LinkAdd",
                        ChainHeader::LinkRemove(header::LinkRemove { .. }) => "LinkRemove",
                        ChainHeader::EntryDelete(header::EntryDelete { .. }) => "EntryDelete",
                        ChainHeader::ChainClose(header::ChainClose { .. }) => "ChainClose",
                        ChainHeader::ChainOpen(header::ChainOpen { .. }) => "ChainOpen",
                        ChainHeader::EntryCreate(header::EntryCreate { .. }) => "EntryCreate",
                        ChainHeader::EntryUpdate(header::EntryUpdate { .. }) => "EntryUpdate",
                    })
                })
                .collect()
                .unwrap();
            assert_eq!(hashes, vec!["EntryCreate", "Dna"]);
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
