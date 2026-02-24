//! Genesis Workflow: Initialize the source chain with the initial entries:
//! - Dna
//! - AgentValidationPkg
//! - AgentId

use super::error::WorkflowError;
use super::error::WorkflowResult;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckInvocationV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckHostAccessV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckInvocationV2;
use crate::core::ribosome::guest_callback::genesis_self_check::{
    GenesisSelfCheckHostAccess, GenesisSelfCheckInvocation, GenesisSelfCheckResult,
};
use crate::{conductor::api::CellConductorApiT, core::ribosome::RibosomeT};
use derive_more::Constructor;
use holochain_sqlite::prelude::*;
use holochain_state::source_chain;
use holochain_types::prelude::*;
use rusqlite::named_params;
use std::sync::Arc;

/// The struct which implements the genesis Workflow
#[derive(Constructor)]
pub struct GenesisWorkflowArgs<Ribosome>
where
    Ribosome: RibosomeT + 'static,
{
    cell_id: CellId,
    membrane_proof: Option<MembraneProof>,
    ribosome: Ribosome,
}

// #[cfg_attr(feature = "instrument", tracing::instrument(skip(workspace, api, args)))]
pub async fn genesis_workflow<Api: CellConductorApiT, Ribosome>(
    mut workspace: GenesisWorkspace,
    api: Api,
    args: GenesisWorkflowArgs<Ribosome>,
) -> WorkflowResult<()>
where
    Ribosome: RibosomeT + 'static,
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
    Ribosome: RibosomeT + 'static,
{
    let GenesisWorkflowArgs {
        cell_id,
        membrane_proof,
        ribosome,
    } = args;

    if workspace
        .has_genesis(cell_id.agent_pubkey().clone())
        .await?
    {
        return Ok(());
    }

    let DnaDef {
        name,
        modifiers: DnaModifiers { properties, .. },
        integrity_zomes,
        ..
    } = &ribosome.dna_def_hashed().content;
    let dna_info = DnaInfoV1 {
        zome_names: integrity_zomes.iter().map(|(n, _)| n.clone()).collect(),
        name: name.clone(),
        hash: cell_id.dna_hash().clone(),
        properties: properties.clone(),
    };
    let result = ribosome
        .run_genesis_self_check(
            GenesisSelfCheckHostAccess {
                host_access_1: GenesisSelfCheckHostAccessV1,
                host_access_2: GenesisSelfCheckHostAccessV2,
            },
            GenesisSelfCheckInvocation {
                invocation_1: GenesisSelfCheckInvocationV1 {
                    payload: Arc::new(GenesisSelfCheckDataV1 {
                        dna_info,
                        membrane_proof: membrane_proof.clone(),
                        agent_key: cell_id.agent_pubkey().clone(),
                    }),
                },
                invocation_2: GenesisSelfCheckInvocationV2 {
                    payload: Arc::new(GenesisSelfCheckDataV2 {
                        membrane_proof: membrane_proof.clone(),
                        agent_key: cell_id.agent_pubkey().clone(),
                    }),
                },
            },
        )
        .await?;

    // If the self-check fails, fail genesis, and don't create the source chain.
    if let GenesisSelfCheckResult::Invalid(reason) = result {
        return Err(WorkflowError::GenesisFailure(reason));
    }

    source_chain::genesis(
        workspace.vault.clone(),
        workspace.dht_db.clone(),
        api.keystore().clone(),
        cell_id.dna_hash().clone(),
        cell_id.agent_pubkey().clone(),
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
    pub fn new(env: DbWrite<DbKindAuthored>, dht_db: DbWrite<DbKindDht>) -> Self {
        Self { vault: env, dht_db }
    }

    pub async fn has_genesis(&self, author: AgentPubKey) -> DatabaseResult<bool> {
        let count = self
            .vault
            .read_async(move |txn| {
                let count: u32 = txn.query_row(
                    "
                SELECT
                COUNT(Action.hash)
                FROM Action
                JOIN DhtOp ON DhtOp.action_hash = Action.hash
                WHERE
                Action.author = :author
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
mod tests {
    use super::*;
    use crate::conductor::api::MockCellConductorApiT;
    use crate::core::ribosome::MockRibosomeT;
    use holochain_keystore::test_keystore;
    use holochain_state::prelude::test_dht_db;
    use holochain_state::{prelude::test_authored_db, source_chain::SourceChain};
    use holochain_trace;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_file;
    use holochain_zome_types::Action;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn genesis_initializes_source_chain() {
        holochain_trace::test_run();
        let test_db = test_authored_db();
        let dht_db = test_dht_db();
        let keystore = test_keystore();
        let vault = test_db.to_db();
        let dna = fake_dna_file("a");
        let author = fake_agent_pubkey_1();

        {
            let workspace = GenesisWorkspace::new(vault.clone(), dht_db.to_db());

            let mut api = MockCellConductorApiT::new();
            api.expect_keystore().return_const(keystore.clone());
            let mut ribosome = MockRibosomeT::new();
            ribosome
                .expect_run_genesis_self_check()
                .returning(|_, _| Ok(GenesisSelfCheckResult::Valid));
            let dna_def = DnaDefHashed::from_content_sync(dna.dna_def().clone());
            ribosome.expect_dna_def_hashed().return_const(dna_def);
            let args = GenesisWorkflowArgs {
                cell_id: CellId::new(dna.dna_hash().clone(), author.clone()),
                membrane_proof: None,
                ribosome,
            };
            let _: () = genesis_workflow(workspace, api, args).await.unwrap();
        }

        {
            let source_chain =
                SourceChain::new(vault.clone(), dht_db.to_db(), keystore, author.clone())
                    .await
                    .unwrap();
            let actions = source_chain
                .query(Default::default())
                .await
                .unwrap()
                .into_iter()
                .map(|e| e.action().clone())
                .collect::<Vec<_>>();

            assert_matches!(
                actions.as_slice(),
                [
                    Action::Dna(_),
                    Action::AgentValidationPkg(_),
                    Action::Create(_)
                ]
            );
        }
    }
}
