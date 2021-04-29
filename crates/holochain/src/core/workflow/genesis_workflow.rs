//! Genesis Workflow: Initialize the source chain with the initial entries:
//! - Dna
//! - AgentValidationPkg
//! - AgentId
//!

// FIXME: understand the details of actually getting the DNA
// FIXME: creating entries in the config db

use super::error::WorkflowError;
use super::error::WorkflowResult;
use crate::conductor::api::CellConductorApiT;
use derive_more::Constructor;
use holochain_sqlite::prelude::*;
use holochain_state::source_chain2;
use holochain_state::workspace::WorkspaceResult;
use holochain_types::prelude::*;
use rusqlite::named_params;
use tracing::*;

/// The struct which implements the genesis Workflow
#[derive(Constructor, Debug)]
pub struct GenesisWorkflowArgs {
    dna_file: DnaFile,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<SerializedBytes>,
}

#[instrument(skip(workspace, api))]
pub async fn genesis_workflow<'env, Api: CellConductorApiT>(
    mut workspace: GenesisWorkspace,
    api: Api,
    args: GenesisWorkflowArgs,
) -> WorkflowResult<()> {
    genesis_workflow_inner(&mut workspace, args, api).await?;
    Ok(())
}

async fn genesis_workflow_inner<Api: CellConductorApiT>(
    workspace: &mut GenesisWorkspace,
    args: GenesisWorkflowArgs,
    api: Api,
) -> WorkflowResult<()> {
    let GenesisWorkflowArgs {
        dna_file,
        agent_pubkey,
        membrane_proof,
    } = args;

    if workspace.has_genesis(&agent_pubkey)? {
        return Ok(());
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

    let keystore = api.keystore();

    let dna_header = Header::Dna(header::Dna {
        author: agent_pubkey.clone(),
        timestamp: timestamp::now(),
        hash: dna_file.dna_hash().clone(),
    });
    let dna_header = HeaderHashed::from_content_sync(dna_header);
    let dna_header = SignedHeaderHashed::new(&keystore, dna_header).await?;
    let dna_header_address = dna_header.as_hash().clone();
    let element = Element::new(dna_header, None);
    let dna_ops = produce_op_lights_from_elements(vec![&element])?;
    let (dna_header, _) = element.into_inner();

    // create the agent validation entry and add it directly to the store
    let agent_validation_header = Header::AgentValidationPkg(header::AgentValidationPkg {
        author: agent_pubkey.clone(),
        timestamp: timestamp::now(),
        header_seq: 1,
        prev_header: dna_header_address,
        membrane_proof,
    });
    let agent_validation_header = HeaderHashed::from_content_sync(agent_validation_header);
    let agent_validation_header =
        SignedHeaderHashed::new(&keystore, agent_validation_header).await?;
    let avh_addr = agent_validation_header.as_hash().clone();
    let element = Element::new(agent_validation_header, None);
    let avh_ops = produce_op_lights_from_elements(vec![&element])?;
    let (agent_validation_header, _) = element.into_inner();

    // create a agent chain element and add it directly to the store
    let agent_header = Header::Create(header::Create {
        author: agent_pubkey.clone(),
        timestamp: timestamp::now(),
        header_seq: 2,
        prev_header: avh_addr,
        entry_type: header::EntryType::AgentPubKey,
        entry_hash: agent_pubkey.clone().into(),
    });
    let agent_header = HeaderHashed::from_content_sync(agent_header);
    let agent_header = SignedHeaderHashed::new(&keystore, agent_header).await?;
    let element = Element::new(agent_header, Some(Entry::Agent(agent_pubkey)));
    let agent_ops = produce_op_lights_from_elements(vec![&element])?;
    let (agent_header, agent_entry) = element.into_inner();
    let agent_entry = agent_entry.into_option();

    workspace.vault.conn()?.with_commit(|txn| {
        source_chain2::put_raw(txn, dna_header, dna_ops, None)?;
        source_chain2::put_raw(txn, agent_validation_header, avh_ops, None)?;
        source_chain2::put_raw(txn, agent_header, agent_ops, agent_entry)?;
        WorkflowResult::Ok(())
    })?;

    Ok(())
}

/// The workspace for Genesis
pub struct GenesisWorkspace {
    vault: EnvWrite,
}

impl GenesisWorkspace {
    /// Constructor
    pub async fn new(env: EnvWrite) -> WorkspaceResult<Self> {
        Ok(Self { vault: env.clone() })
    }

    pub fn has_genesis(&self, author: &AgentPubKey) -> DatabaseResult<bool> {
        let count = self.vault.conn()?.with_reader(|txn| {
            let count: u32 = txn.query_row(
                "
                SELECT 
                COUNT(Header.hash)
                FROM Header
                JOIN DhtOp ON DhtOp.header_hash = Header.hash
                WHERE
                DhtOp.is_authored = 1
                AND
                Header.author = :author
                ",
                named_params! {
                    ":author": author,
                },
                |row| row.get(0),
            )?;
            DatabaseResult::Ok(count)
        })?;
        Ok(count >= 3)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use crate::conductor::api::MockCellConductorApi;
    use crate::core::SourceChainResult;
    use fallible_iterator::FallibleIterator;
    use holochain_state::{prelude::test_cell_env, source_chain::SourceChain};
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
    async fn genesis_initializes_source_chain() {
        observability::test_run().unwrap();
        let test_env = test_cell_env();
        let arc = test_env.env();
        let dna = fake_dna_file("a");
        let agent_pubkey = fake_agent_pubkey_1();

        {
            let workspace = GenesisWorkspace::new(arc.clone().into()).await.unwrap();
            let mut api = MockCellConductorApi::new();
            api.expect_sync_dpki_request()
                .returning(|_, _| Ok("mocked dpki request response".to_string()));
            let args = GenesisWorkflowArgs {
                dna_file: dna.clone(),
                agent_pubkey: agent_pubkey.clone(),
                membrane_proof: None,
            };
            let _: () = genesis_workflow(workspace, api, args).await.unwrap();
        }

        {
            let source_chain = SourceChain::new(arc.clone().into()).unwrap();
            assert_eq!(source_chain.agent_pubkey().unwrap(), agent_pubkey);
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
                [Header::Create(_), Header::AgentValidationPkg(_), Header::Dna(_)]
            );
        }
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

- initialize databases, save to conductor runtime config.

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
