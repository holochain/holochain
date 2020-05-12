//! Genesis Workflow: Initialize the source chain with the initial entries:
//! - Dna
//! - AgentValidationPkg
//! - AgentId
//!

// FIXME: understand the details of actually getting the DNA
// FIXME: creating entries in the config db

use super::Workspace;
use super::{Workflow, WorkflowEffects, WorkflowError, WorkflowResult};
use crate::conductor::api::CellConductorApiT;
use crate::core::state::{source_chain::SourceChainBuf, workspace::WorkspaceResult};
use futures::future::FutureExt;
use holochain_state::prelude::*;
use holochain_types::prelude::*;
use holochain_types::{dna::DnaFile, entry::Entry, header, Header};
use must_future::MustBoxFuture;

/// The struct which implements the genesis Workflow
pub struct GenesisWorkflow<Api: CellConductorApiT> {
    api: Api,
    dna_file: DnaFile,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<SerializedBytes>,
}

impl<'env, Api: CellConductorApiT + Send + Sync + 'env> Workflow<'env> for GenesisWorkflow<Api> {
    type Output = ();
    type Workspace = GenesisWorkspace<'env>;
    type Triggers = ();

    fn workflow(
        self,
        mut workspace: Self::Workspace,
    ) -> MustBoxFuture<'env, WorkflowResult<'env, Self>> {
        async {
            let Self {
                api,
                dna_file,
                agent_pubkey,
                membrane_proof,
            } = self;

            // TODO: this is a placeholder for a real DPKI request to show intent
            if api
                .dpki_request("is_agent_pubkey_valid".into(), agent_pubkey.to_string())
                .await
                .map_err(Box::new)?
                == "INVALID"
            {
                return Err(WorkflowError::AgentInvalid(agent_pubkey.clone()));
            }

            // create a DNA chain element and add it directly to the store
            let dna_header = Header::Dna(header::Dna {
                timestamp: Timestamp::now(),
                author: agent_pubkey.clone(),
                hash: dna_file.dna_hash().clone(),
            });
            let dna_header_address = workspace.source_chain.put(dna_header.clone(), None).await?;

            // create the agent validation entry and add it directly to the store
            let agent_validation_header = Header::AgentValidationPkg(header::AgentValidationPkg {
                timestamp: Timestamp::now(),
                author: agent_pubkey.clone(),
                prev_header: dna_header_address,
                membrane_proof,
            });
            let avh_addr = workspace
                .source_chain
                .put(agent_validation_header.clone(), None)
                .await?;

            // create a agent chain element and add it directly to the store
            let agent_header = Header::EntryCreate(header::EntryCreate {
                timestamp: Timestamp::now(),
                author: agent_pubkey.clone(),
                prev_header: avh_addr,
                entry_type: header::EntryType::AgentPubKey,
                entry_address: agent_pubkey.clone().into(),
            });
            workspace
                .source_chain
                .put(agent_header, Some(Entry::Agent(agent_pubkey)))
                .await?;

            let fx = WorkflowEffects {
                workspace,
                callbacks: Default::default(),
                signals: Default::default(),
                triggers: (),
            };
            let result = ();

            Ok((result, fx))
        }
        .boxed()
        .into()
    }
}

/// The workspace for Genesis
pub struct GenesisWorkspace<'env> {
    source_chain: SourceChainBuf<'env, Reader<'env>>,
}

impl<'env> GenesisWorkspace<'env> {
    /// Constructor
    #[allow(dead_code)]
    pub fn new(reader: &'env Reader<'env>, dbs: &impl GetDb) -> WorkspaceResult<Self> {
        Ok(Self {
            source_chain: SourceChainBuf::<'env>::new(reader, dbs)?,
        })
    }
}

impl<'env> Workspace<'env> for GenesisWorkspace<'env> {
    fn commit_txn(self, mut writer: Writer) -> WorkspaceResult<()> {
        self.source_chain.flush_to_txn(&mut writer)?;
        writer.commit()?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {

    use super::{GenesisWorkflow, GenesisWorkspace};
    use crate::core::workflow::run_workflow;
    use crate::{conductor::api::MockCellConductorApi, core::state::source_chain::SourceChain};
    use fallible_iterator::FallibleIterator;
    use holo_hash::Hashed;
    use holochain_state::{env::*, prelude::Readable, test_utils::test_cell_env};
    use holochain_types::{
        entry::Entry,
        header, observability,
        test_utils::{fake_agent_pubkey_1, fake_dna_file},
        Header, Timestamp,
    };
    use matches::assert_matches;

    pub async fn fake_genesis<R: Readable>(source_chain: &mut SourceChain<'_, R>) -> Header {
        let agent_pubkey = fake_agent_pubkey_1();
        let agent_entry = Entry::Agent(agent_pubkey.clone());
        let dna = fake_dna_file("cool dna");
        let dna_header = Header::Dna(header::Dna {
            timestamp: Timestamp::now(),
            author: agent_pubkey.clone(),
            hash: dna.dna_hash().clone(),
        });
        let dna_hash = source_chain.put(dna_header, None).await.unwrap();

        // create the agent validation entry and add it directly to the store
        let agent_validation_header = Header::AgentValidationPkg(header::AgentValidationPkg {
            timestamp: Timestamp::now(),
            author: agent_pubkey.clone(),
            prev_header: dna_hash,
            membrane_proof: None,
        });
        let avh_hash = source_chain
            .put(agent_validation_header, None)
            .await
            .unwrap();

        let agent_header = Header::EntryCreate(header::EntryCreate {
            timestamp: Timestamp::now(),
            author: agent_pubkey.clone(),
            prev_header: avh_hash,
            entry_type: header::EntryType::AgentPubKey,
            entry_address: agent_pubkey.clone().into(),
        });
        source_chain
            .put(agent_header.clone(), Some(agent_entry))
            .await
            .unwrap();

        agent_header
    }

    #[tokio::test(threaded_scheduler)]
    async fn genesis_initializes_source_chain() -> Result<(), anyhow::Error> {
        observability::test_run()?;
        let arc = test_cell_env();
        let env = arc.guard().await;
        let dbs = arc.dbs().await;
        let dna = fake_dna_file("a");
        let agent_pubkey = fake_agent_pubkey_1();

        {
            let reader = env.reader()?;
            let workspace = GenesisWorkspace::new(&reader, &dbs)?;
            let mut api = MockCellConductorApi::new();
            api.expect_sync_dpki_request()
                .returning(|_, _| Ok("mocked dpki request response".to_string()));
            let workflow = GenesisWorkflow {
                api,
                dna_file: dna.clone(),
                agent_pubkey: agent_pubkey.clone(),
                membrane_proof: None,
            };
            let _: () = run_workflow(arc.clone(), workflow, workspace).await?;
        }

        {
            let reader = env.reader()?;

            let source_chain = SourceChain::new(&reader, &dbs)?;
            assert_eq!(source_chain.agent_pubkey()?, agent_pubkey);
            source_chain.chain_head().expect("chain head should be set");

            let mut iter = source_chain.iter_back();
            let mut headers = Vec::new();

            while let Some(addr) = iter.next().unwrap() {
                if let Ok(Some(h)) = source_chain.get_header(&addr).await {
                    let (h, _) = h.into_inner();
                    let (h, _) = h.into_inner();
                    headers.push(h);
                }
            }

            assert_matches!(
                headers.as_slice(),
                [Header::EntryCreate(_), Header::AgentValidationPkg(_), Header::Dna(_)]
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
