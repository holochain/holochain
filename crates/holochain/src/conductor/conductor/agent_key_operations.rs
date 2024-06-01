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
        let result =
            Conductor::revoke_agent_key_for_app_inner(self.clone(), agent_key, app_id.clone())
                .await;

        // Enable app again.
        self.clone().enable_app(app_id).await?;

        result
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
                println!("cell id {cell_id:?}");
                let conductor = conductor.clone();
                let agent_key = agent_key.clone();
                async move {
                    let authored_db = conductor
                        .get_or_create_authored_db(cell_id.dna_hash(), agent_key.clone())
                        .unwrap();
                    let create_agent_key_address = authored_db
                        .read_async({
                            let agent_key = agent_key.clone();
                            move |txn| {
                                let agent_key_entry_hash: EntryHash = agent_key.clone().into();
                                let create_agent_key_address = txn.query_row(
                                    "SELECT hash FROM Action WHERE author = :agent_key AND type = :create AND entry_hash = :agent_key_entry_hash",
                                    named_params! {":agent_key": agent_key, ":create": ActionType::Create.to_string(), ":agent_key_entry_hash": agent_key_entry_hash},
                                    |row| {
                                        row.get::<_, ActionHash>("hash")
                                    },
                                )?;
                                Ok::<_, DatabaseError>(create_agent_key_address)
                            }
                        })
                        .await?;
                    println!("create agent key address is {create_agent_key_address:?}");
                    let source_chain = SourceChain::new(authored_db, conductor.get_dht_db(cell_id.dna_hash())?, conductor.get_dht_db_cache(cell_id.dna_hash())?, conductor.keystore().clone(), agent_key.clone()).await?;
                    let result = source_chain.put_weightless(builder::Delete::new(create_agent_key_address, agent_key.clone().into()), None, ChainTopOrdering::Strict).await?;
                    let network = conductor
                        .holochain_p2p
                        .to_dna(cell_id.dna_hash().clone(), conductor.get_chc(&cell_id));
                    source_chain.flush(&network).await?;
                    println!("result of put {result:?}");
                    Ok::<_, ConductorApiError>(())
                }
            });
        let result = futures::future::join_all(delete_agent_key_of_all_cells).await;
        println!("all delte futures result {result:?}");

        Ok(())
    }
}
