use holochain_types::deepkey_roundtrip_backward;

use super::*;

pub type RevokeAgentKeyForAppResult = HashMap<CellId, ConductorApiResult<()>>;

impl Conductor {
    /// Revoke an agent's key pair for all cells of an app.
    ///
    /// Writes a `Delete` action to the source chain of all cells of the app, which renders them read-only.
    /// If DPKI is installed as conductor service, the agent key will be revoked there too and becomes
    /// invalid.
    pub async fn revoke_agent_key_for_app(
        self: Arc<Self>,
        agent_key: AgentPubKey,
        app_id: InstalledAppId,
    ) -> ConductorResult<RevokeAgentKeyForAppResult> {
        // Disable app while revoking key
        self.clone()
            .disable_app(app_id.clone(), DisabledAppReason::DeletingAgentKey)
            .await?;

        // Revoke key in DPKI first, if installed, and then in cells' source chains.
        // Call separate function so that in case a part of key revocation fails, the app is still enabled again.
        let revocation_per_cell_results =
            Conductor::revoke_agent_key_for_app_inner(self.clone(), agent_key, app_id.clone())
                .await;

        // Enable app again.
        self.clone().enable_app(app_id.clone()).await?;

        let deletion_per_cell_results = revocation_per_cell_results?;

        // Publish 'Delete' actions of cells where successful.
        // Triggering workflow is only possible when cells are enabled.
        let publish_workflow_triggers = deletion_per_cell_results
            .iter()
            .filter(|(_, result)| result.is_ok())
            .map({
                |(cell_id, _)| {
                    let conductor = self.clone();
                    async move {
                        let cell_triggers = conductor.get_cell_triggers(&cell_id).await?;
                        cell_triggers.publish_dht_ops.trigger(&"delete agent key");
                        ConductorApiResult::Ok(())
                    }
                }
            });
        let _trigger_publish_results = futures::future::join_all(publish_workflow_triggers).await;

        // Return cell ids with their agent key deletion result
        Ok(deletion_per_cell_results)
    }

    async fn revoke_agent_key_for_app_inner(
        conductor: Arc<Conductor>,
        agent_key: AgentPubKey,
        app_id: InstalledAppId,
    ) -> ConductorResult<RevokeAgentKeyForAppResult> {
        // If DPKI service is installed, revoke agent key there first
        if let Some(dpki_service) = conductor.running_services().dpki {
            let dpki_state = dpki_service.state().await;
            let timestamp = Timestamp::now();
            let key_state = dpki_state.key_state(agent_key.clone(), timestamp).await?;
            match key_state {
                KeyState::NotFound => {
                    return Err(ConductorError::DpkiError(
                        DpkiServiceError::DpkiAgentMissing(agent_key.clone()),
                    ))
                }
                // If the key already is invalid, do nothing. Operation should be idempotent to allow for
                // retries if agent key of some source chain could not be deleted successfully.
                KeyState::Invalid(_) => (),
                KeyState::Valid(_) => {
                    // Get action hash of key registration
                    let key_meta = dpki_state.query_key_meta(agent_key.clone()).await?;
                    // Sign revocation request
                    let signature = dpki_service
                        .cell_id
                        .agent_pubkey()
                        .sign_raw(
                            &conductor.keystore,
                            key_meta.key_registration_addr.get_raw_39().into(),
                        )
                        .await
                        .map_err(|e| DpkiServiceError::Lair(e.into()))?;
                    let signature = deepkey_roundtrip_backward!(Signature, &signature);
                    // Revoke key in DPKI
                    let _revocation = dpki_state
                        .revoke_key(RevokeKeyInput {
                            key_revocation: KeyRevocation {
                                prior_key_registration: key_meta.key_registration_addr,
                                revocation_authorization: vec![(0, signature)],
                            },
                        })
                        .await?;
                }
            };
        }

        // Write 'Delete' action to source chains of all cells of the app
        let state = conductor.get_state().await?;
        let app = state.get_app(&app_id)?;
        if *app.agent_key() != agent_key {
            return Err(ConductorError::AppError(AppError::AgentKeyMissing(
                agent_key, app_id,
            )));
        }
        let all_cells: Vec<CellId> = app.all_cells().collect();
        let delete_agent_key_of_all_cells = all_cells.clone().into_iter().map(|cell_id| {
            let conductor = conductor.clone();
            let agent_key = agent_key.clone();
            async move {
                // Instantiate source chain
                let authored_db = conductor
                    .get_or_create_authored_db(cell_id.dna_hash(), agent_key.clone())
                    .unwrap();
                let source_chain = SourceChain::new(
                    authored_db,
                    conductor.get_dht_db(cell_id.dna_hash())?,
                    conductor.get_dht_db_cache(cell_id.dna_hash())?,
                    conductor.keystore().clone(),
                    agent_key.clone(),
                )
                .await?;

                // Insert `Delete` action of agent pub key into source chain
                source_chain.delete_valid_agent_pub_key().await?;
                let network = conductor
                    .holochain_p2p
                    .to_dna(cell_id.dna_hash().clone(), conductor.get_chc(&cell_id));
                source_chain.flush(&network).await?;

                Ok::<_, ConductorApiError>(())
            }
        });
        let delete_agent_key_results =
            futures::future::join_all(delete_agent_key_of_all_cells).await;
        // Build result map with cell id as key and deletion result as value
        let mut cell_results = HashMap::new();
        delete_agent_key_results
            .into_iter()
            .enumerate()
            .for_each(|(index, result)| {
                cell_results.insert(all_cells[index].clone(), result);
            });

        Ok(cell_results)
    }
}
