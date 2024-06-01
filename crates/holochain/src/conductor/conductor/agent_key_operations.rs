use holochain_types::deepkey_roundtrip_backward;
use holochain_zome_types::action::builder;

use super::*;

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
    ) -> ConductorResult<()> {
        // Disable app while revoking key.
        self.clone()
            .disable_app(app_id.clone(), DisabledAppReason::DeleteAgentKey)
            .await?;

        // Revoke key in DPKI first, if installed, and then in cells' source chains.
        // Call separate function so that in case a part of key revocation fails, the app is still enabled again.
        let revocation_result =
            Conductor::revoke_agent_key_for_app_inner(self.clone(), agent_key, app_id.clone())
                .await;

        // Enable app again.
        self.clone().enable_app(app_id).await?;

        revocation_result
    }

    async fn revoke_agent_key_for_app_inner(
        conductor: Arc<Conductor>,
        agent_key: AgentPubKey,
        app_id: InstalledAppId,
    ) -> ConductorResult<()> {
        // If DPKI service is installed, revoke agent key there first.
        let dpki_service = conductor
            .running_services()
            .dpki
            .ok_or(ConductorError::DpkiError(
                DpkiServiceError::DpkiNotInstalled,
            ))?;
        let dpki_state = dpki_service.state().await;
        let timestamp = Timestamp::now();
        let key_state = dpki_state
            .key_state(agent_key.clone(), timestamp.clone())
            .await?;
        match key_state {
            KeyState::NotFound => {
                return Err(ConductorError::DpkiError(
                    DpkiServiceError::DpkiAgentMissing(agent_key.clone()),
                ))
            }
            KeyState::Invalid(_) => {
                return Err(ConductorError::DpkiError(
                    DpkiServiceError::DpkiAgentInvalid(agent_key.clone(), timestamp),
                ))
            }
            KeyState::Valid(_) => {
                // get action hash of key registration
                let key_meta = dpki_state.query_key_meta(agent_key.clone()).await?;
                // sign revocation request
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

        // Write 'Delete' action to source chains of all cells of the app.
        let state = conductor.get_state().await?;
        let app = state.get_app(&app_id)?;
        let delete_agent_key_of_all_cells = app.all_cells().map(|cell_id| {
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

                // Query source chain for agent pub key 'Create' action.
                let mut entry_hashes = HashSet::new();
                entry_hashes.insert(agent_key.clone().into());
                let queried = source_chain
                    .query(ChainQueryFilter {
                        sequence_range: ChainQueryFilterRange::Unbounded,
                        entry_type: None,
                        entry_hashes: Some(entry_hashes),
                        action_type: Some(vec![ActionType::Create]),
                        include_entries: false,
                        order_descending: false,
                    })
                    .await?;
                // There must only be 1 record of the agent pub key 'Create' action.
                assert!(queried.len() == 1);
                let create_agent_key_address = queried[0].action_address().clone();

                // Insert `Delete` action of agent pub key into source chain.
                let _ = source_chain
                    .put_weightless(
                        builder::Delete::new(create_agent_key_address, agent_key.clone().into()),
                        None,
                        ChainTopOrdering::Strict,
                    )
                    .await?;
                let network = conductor
                    .holochain_p2p
                    .to_dna(cell_id.dna_hash().clone(), conductor.get_chc(&cell_id));
                source_chain.flush(&network).await?;

                // Trigger publish for 'Delete" actions.
                let cell_triggers = conductor.get_cell_triggers(&cell_id).await?;
                cell_triggers
                    .publish_dht_ops
                    .trigger(&"agent_key_revocation");

                Ok::<_, ConductorApiError>(())
            }
        });
        let _ = futures::future::join_all(delete_agent_key_of_all_cells).await;

        Ok(())
    }
}
