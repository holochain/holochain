//! Genesis Workflow: Initialize the source chain with the initial entries:
//! - Dna
//! - AgentValidationPkg
//! - AgentId

use super::error::WorkflowError;
use super::error::WorkflowResult;
use crate::conductor::api::CellConductorApiT;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckHostAccessV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v1::GenesisSelfCheckInvocationV1;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckHostAccessV2;
use crate::core::ribosome::guest_callback::genesis_self_check::v2::GenesisSelfCheckInvocationV2;
use crate::core::ribosome::guest_callback::genesis_self_check::{
    GenesisSelfCheckHostAccess, GenesisSelfCheckInvocation, GenesisSelfCheckResult,
};
use crate::core::ribosome::Ribosome;
use derive_more::Constructor;
use holochain_state::dht_store::DhtStore;
use holochain_state::prelude::StateQueryResult;
use holochain_state::source_chain;
use holochain_types::prelude::*;
use std::sync::Arc;

/// The struct which implements the genesis Workflow
#[derive(Constructor)]
pub struct GenesisWorkflowArgs {
    cell_id: CellId,
    membrane_proof: Option<MembraneProof>,
    ribosome: Ribosome,
}

// #[cfg_attr(feature = "instrument", tracing::instrument(skip(workspace, api, args)))]
pub async fn genesis_workflow<Api: CellConductorApiT>(
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
    } = &ribosome.dna_def().content;
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
        workspace.dht_store.clone(),
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
    dht_store: DhtStore,
}

impl GenesisWorkspace {
    /// Constructor
    pub fn new(dht_store: DhtStore) -> Self {
        Self { dht_store }
    }

    pub async fn has_genesis(&self, author: AgentPubKey) -> StateQueryResult<bool> {
        self.dht_store.as_read().has_genesis(&author).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conductor::api::MockCellConductorApiT;
    use crate::core::ribosome::mock_ribosome::MockRibosomeBuilder;
    use holochain_keystore::test_keystore;
    use holochain_state::source_chain::SourceChain;
    use holochain_trace;
    use holochain_types::test_utils::fake_agent_pubkey_1;
    use holochain_types::test_utils::fake_dna_file;
    use holochain_zome_types::Action;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    async fn has_genesis() {
        holochain_trace::test_run();
        let keystore = test_keystore();
        let dna = fake_dna_file("b");
        let author = fake_agent_pubkey_1();
        let dht_store = holochain_state::test_utils::test_dht_store(dna.dna_hash().clone()).await;

        let workspace = GenesisWorkspace::new(dht_store.clone());

        // Before genesis the store has none of the author's actions.
        assert!(!workspace.has_genesis(author.clone()).await.unwrap());

        let mut api = MockCellConductorApiT::new();
        api.expect_keystore().return_const(keystore.clone());
        let ribosome = MockRibosomeBuilder::new_with_dna_def(dna.dna_def_hashed().clone())
            .with_genesis_self_check_handler(|_, _| Ok(GenesisSelfCheckResult::Valid))
            .build()
            .await
            .unwrap();
        let args = GenesisWorkflowArgs {
            cell_id: CellId::new(dna.dna_hash().clone(), author.clone()),
            membrane_proof: None,
            ribosome,
        };
        genesis_workflow(workspace, api, args).await.unwrap();

        // After genesis the store has the three genesis actions, so a fresh
        // workspace over the same store reports genesis complete.
        let workspace = GenesisWorkspace::new(dht_store);
        assert!(workspace.has_genesis(author).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn genesis_initializes_source_chain() {
        holochain_trace::test_run();
        let keystore = test_keystore();
        let dna = fake_dna_file("a");
        let author = fake_agent_pubkey_1();

        let dht_store = holochain_state::test_utils::test_dht_store(dna.dna_hash().clone()).await;

        {
            let workspace = GenesisWorkspace::new(dht_store.clone());

            let mut api = MockCellConductorApiT::new();
            api.expect_keystore().return_const(keystore.clone());
            let dna_def = DnaDefHashed::from_content_sync(dna.dna_def().clone());
            let ribosome = MockRibosomeBuilder::new_with_dna_def(dna_def)
                .with_genesis_self_check_handler(|_, _| Ok(GenesisSelfCheckResult::Valid))
                .build()
                .await
                .unwrap();

            let args = GenesisWorkflowArgs {
                cell_id: CellId::new(dna.dna_hash().clone(), author.clone()),
                membrane_proof: None,
                ribosome,
            };
            let _: () = genesis_workflow(workspace, api, args).await.unwrap();
        }

        {
            let dht_store = dht_store;
            let source_chain = SourceChain::new(dht_store, keystore, author.clone())
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
