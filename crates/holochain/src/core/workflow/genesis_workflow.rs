//! Genesis Workflow: Initialize the source chain with the initial entries:
//! - Dna
//! - AgentValidationPkg
//! - AgentId
//!

use std::sync::Arc;

use super::error::WorkflowError;
use super::error::WorkflowResult;
use crate::core::ribosome::guest_callback::genesis_self_check::{
    GenesisSelfCheckHostAccess, GenesisSelfCheckInvocation, GenesisSelfCheckResult,
};
use crate::{conductor::api::CellConductorApiT, core::ribosome::RibosomeT};
use derive_more::Constructor;
use holochain_sqlite::prelude::*;
use holochain_state::source_chain;
use holochain_state::workspace::WorkspaceResult;
use holochain_types::db_cache::DhtDbQueryCache;
use holochain_types::prelude::*;
use rusqlite::named_params;
use tracing::*;

/// The struct which implements the genesis Workflow
#[derive(Constructor)]
pub struct GenesisWorkflowArgs<Ribosome>
where
    Ribosome: RibosomeT + Send + 'static,
{
    dna_file: DnaFile,
    agent_pubkey: AgentPubKey,
    membrane_proof: Option<MembraneProof>,
    ribosome: Ribosome,
    dht_db_cache: DhtDbQueryCache,
}

#[instrument(skip(workspace, api, args))]
pub async fn genesis_workflow<'env, Api: CellConductorApiT, Ribosome>(
    mut workspace: GenesisWorkspace,
    api: Api,
    args: GenesisWorkflowArgs<Ribosome>,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + Send + 'static,
{
    genesis_workflow_inner(&mut workspace, args, api).await?;
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
        dht_db_cache,
    } = args;

    if workspace.has_genesis(agent_pubkey.clone()).await? {
        return Ok(());
    }

    let dna_hash = ribosome.dna_def().to_hash();
    let DnaDef {
        name,
        properties,
        zomes,
        ..
    } = &ribosome.dna_def().content;
    let dna_info = DnaInfo {
        zome_names: zomes.iter().map(|(n, _)| n.clone()).collect(),
        name: name.clone(),
        hash: dna_hash,
        properties: properties.clone(),
    };
    let result = ribosome.run_genesis_self_check(
        GenesisSelfCheckHostAccess,
        GenesisSelfCheckInvocation {
            payload: Arc::new(GenesisSelfCheckData {
                dna_info,
                membrane_proof: membrane_proof.clone(),
                agent_key: agent_pubkey.clone(),
            }),
        },
    )?;

    // If the self-check fails, fail genesis, and don't create the source chain.
    if let GenesisSelfCheckResult::Invalid(reason) = result {
        return Err(WorkflowError::GenesisFailure(reason));
    }

    // NB: this is just a placeholder for a real DPKI request to show intent
    if api
        .dpki_request("is_agent_pubkey_valid".into(), agent_pubkey.to_string())
        .await
        .expect("TODO: actually implement this")
        == "INVALID"
    {
        return Err(WorkflowError::AgentInvalid(agent_pubkey.clone()));
    }

    source_chain::genesis(
        workspace.vault.clone(),
        workspace.dht_db.clone(),
        &dht_db_cache,
        api.keystore().clone(),
        dna_file.dna_hash().clone(),
        agent_pubkey,
        membrane_proof,
    )
    .await?;

    Ok(())
}

/// The workspace for Genesis
pub struct GenesisWorkspace {
    vault: DbWrite<DbKindAuthored>,
    dht_db: DbWrite<DbKindDht>,
}

impl GenesisWorkspace {
    /// Constructor
    pub fn new(env: DbWrite<DbKindAuthored>, dht_db: DbWrite<DbKindDht>) -> WorkspaceResult<Self> {
        Ok(Self { vault: env, dht_db })
    }

    pub async fn has_genesis(&self, author: AgentPubKey) -> DatabaseResult<bool> {
        let count = self
            .vault
            .async_reader(move |txn| {
                let count: u32 = txn.query_row(
                    "
                SELECT
                COUNT(Header.hash)
                FROM Header
                JOIN DhtOp ON DhtOp.header_hash = Header.hash
                WHERE
                Header.author = :author
                LIMIT 3
                ",
                    named_params! {
                        ":author": author,
                    },
                    |row| row.get(0),
                )?;
                DatabaseResult::Ok(count)
            })
            .await?;
        Ok(count >= 3)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use crate::conductor::api::MockCellConductorApiT;
    use crate::core::ribosome::MockRibosomeT;
    use futures::FutureExt;
    use holochain_state::prelude::test_dht_db;
    use holochain_state::{prelude::test_authored_db, source_chain::SourceChain};
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_file;
    use holochain_zome_types::Header;
    use matches::assert_matches;
    use observability;

    #[tokio::test(flavor = "multi_thread")]
    async fn genesis_initializes_source_chain() {
        observability::test_run().unwrap();
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let dht_db_cache = DhtDbQueryCache::new(dht_db.to_db().into());
        let keystore = test_keystore();
        let vault = test_db.to_db();
        let dna = fake_dna_file("a");
        let author = fake_agent_pubkey_1();

        {
            let workspace = GenesisWorkspace::new(vault.clone().into(), dht_db.to_db()).unwrap();
            let mut api = MockCellConductorApiT::new();
            api.expect_dpki_request().returning(|_, _| {
                async move { Ok("mocked dpki request response".to_string()) }.boxed()
            });
            api.expect_keystore().return_const(keystore.clone());
            let mut ribosome = MockRibosomeT::new();
            ribosome
                .expect_run_genesis_self_check()
                .returning(|_, _| Ok(GenesisSelfCheckResult::Valid));
            let dna_def = DnaDefHashed::from_content_sync(dna.dna_def().clone());
            ribosome.expect_dna_def().return_const(dna_def);
            let args = GenesisWorkflowArgs {
                dna_file: dna.clone(),
                agent_pubkey: author.clone(),
                membrane_proof: None,
                ribosome,
                dht_db_cache: dht_db_cache.clone(),
            };
            let _: () = genesis_workflow(workspace, api, args).await.unwrap();
        }

        {
            let source_chain = SourceChain::new(
                vault.clone(),
                dht_db.to_db(),
                dht_db_cache,
                keystore,
                author.clone(),
            )
            .await
            .unwrap();
            let headers = source_chain
                .query(Default::default())
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.header().clone())
                .collect::<Vec<_>>();

            assert_matches!(
                headers.as_slice(),
                [
                    Header::Dna(_),
                    Header::AgentValidationPkg(_),
                    Header::Create(_)
                ]
            );
        }
    }
}

/* TODO: update and rewrite as proper rust docs

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
