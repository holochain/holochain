use crate::conductor::space::Space;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::{WorkflowError, WorkflowResult};
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_p2p::DynHolochainP2pDna;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::prelude::*;
use holochain_timestamp::Timestamp;
use holochain_types::dht_op::ChainOp;
use holochain_zome_types::prelude::SignedAction;

pub(crate) async fn inner_countersigning_session_complete(
    space: Space,
    network: DynHolochainP2pDna,
    keystore: MetaLairClient,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<Option<EntryHash>> {
    // Using iterators is fine in this function as there can only be a maximum of 8 actions.
    let (this_cells_action_hash, entry_hash) = match signed_actions
        .iter()
        .find(|a| *a.author() == author)
        .and_then(|sa| {
            sa.entry_hash()
                .cloned()
                .map(|eh| (ActionHash::with_data_sync(sa), eh))
        }) {
        Some(a) => a,
        None => return Ok(None),
    };

    // Read the chain lock from the merged store (#5370). It isn't necessarily for
    // the current session; we can't check that until we have the session data.
    let chain_lock = space
        .dht_store
        .as_read()
        .get_chain_lock(author.clone())
        .await?;

    // Read the current countersigning session from the merged store (#5370). A
    // `Some` result guarantees the `CounterSign` entry is present in the store,
    // so no separate entry-presence check is needed.
    let maybe_current_session = space
        .dht_store
        .as_read()
        .current_countersigning_session(&author)
        .await?;

    // Do a quick check to see if this entry hash matches the current locked session, so we don't
    // check signatures unless there is an active session.
    let maybe_matched_session = match maybe_current_session {
        Some((session_record, cs_entry_hash, session_data)) => {
            let lock_subject = session_data.preflight_request.fingerprint()?;
            match &chain_lock {
                // This is the case where we have already locked the chain for another session and are
                // receiving another signature bundle from a different session. We don't need this, so
                // it's safe to short circuit.
                Some(chain_lock)
                    if cs_entry_hash == entry_hash && chain_lock.subject() == lock_subject =>
                {
                    Some((session_record, session_data))
                }
                _ => None,
            }
        }
        None => None,
    };

    let (session_record, session_data) = match maybe_matched_session {
        Some(cs) => cs,
        None => {
            // If there is no active session then we can short circuit.
            tracing::info!("Received a signature bundle for a session that exists in state but is missing from the database");
            return Ok(None);
        }
    };

    // Verify signatures of actions.
    let mut i_am_an_author = false;
    for sa in &signed_actions {
        if !sa
            .action()
            .author()
            .verify_signature(sa.signature(), sa.action())
            .await?
        {
            tracing::warn!("Invalid signature found: {:?}", sa);
            return Ok(None);
        }
        if sa.action().author() == &author {
            i_am_an_author = true;
        }
    }

    // Countersigning success is ultimately between authors to agree and publish.
    if !i_am_an_author {
        // We're effectively rejecting this signature bundle but communicating that this signature
        // bundle wasn't acceptable so that we can try another one.
        tracing::debug!(
            "I am not an author for this countersigning session, rejecting signature bundle"
        );
        return Ok(None);
    }

    // Hash actions.
    let incoming_actions: Vec<_> = signed_actions
        .iter()
        .map(ActionHash::with_data_sync)
        .collect();

    let mut integrity_check_passed = false;

    let weight = weigh_placeholder();
    let stored_actions = session_data.build_action_set(entry_hash, weight)?;
    if stored_actions.len() == incoming_actions.len() {
        tracing::debug!("Have the right number of actions");

        // Check all stored action hashes match an incoming action hash.
        if stored_actions.iter().all(|a| {
            let a = ActionHash::with_data_sync(a);
            incoming_actions.contains(&a)
        }) {
            tracing::debug!("All hashes are correct");
            // All checks have passed, proceed to update the session state.
            integrity_check_passed = true;
        }
    }

    if !integrity_check_passed {
        // If the integrity check fails then we can't proceed with this signature bundle.
        tracing::debug!("Integrity check failed for countersigning session");
        return Ok(None);
    }

    reveal_countersigning_session(
        space,
        network.clone(),
        keystore,
        session_record,
        &author,
        this_cells_action_hash,
        integration_trigger,
        publish_trigger,
    )
    .await?;

    // Publish other signers agent activity ops to their agent activity authorities.
    for sa in signed_actions {
        let (action, signature) = sa.into();
        if *action.author() == author {
            continue;
        }
        let op = ChainOp::RegisterAgentActivity(signature, action);
        let basis = op.dht_basis();
        if let Err(e) = network.publish_countersign(basis, op).await {
            tracing::error!(
                "Failed to publish to other counter-signers agent authorities because of: {:?}",
                e
            );
        }
    }

    tracing::info!(
        "Countersigning session complete for agent {:?} in approximately {}ms",
        author,
        (Timestamp::now() - session_data.preflight_request.session_times.start)
            .unwrap_or_default()
            .num_milliseconds()
    );

    Ok(Some(session_data.preflight_request.app_entry_hash))
}

#[allow(clippy::too_many_arguments)]
async fn reveal_countersigning_session(
    space: Space,
    network: DynHolochainP2pDna,
    keystore: MetaLairClient,
    session_record: Record,
    author: &AgentPubKey,
    this_cells_action_hash: ActionHash,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<()> {
    apply_success_state_changes(space, author, this_cells_action_hash, integration_trigger).await?;

    publish_trigger.trigger(&"publish countersigning_success");

    Ok(())
}

async fn apply_success_state_changes(
    space: Space,
    author: &AgentPubKey,
    this_cells_action_hash: ActionHash,
    integration_trigger: TriggerSender,
) -> Result<(), WorkflowError> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;
    let dht_db = space.dht_db.clone();

    // Load the op hashes for this session so that we can publish them. #5370:
    // read the session's ops from the merged store (now the sole source-chain
    // write) instead of the legacy authored DB. The session's withhold-publish
    // flag is cleared on the merged store below via `clear_op_withhold_publishes`.
    let this_cell_actions_op_basis_hashes = space
        .dht_store
        .as_read()
        .chain_op_hashes_for_action(this_cells_action_hash)
        .await?;

    // All checks have passed so unlock the chain. #5370: the lock lives in the
    // merged store. The withhold-publish flag is cleared on the merged store
    // below via `clear_op_withhold_publishes`.
    space
        .dht_store
        .release_chain_lock(author)
        .await
        .map_err(WorkflowError::from)?;

    // #5370: legacy authored->dht_db integration. The source chain is no longer
    // written to the authored DB, so this is now a no-op (the ops are not found
    // in the authored DB) and remains only until the legacy DbKindDht reader is
    // retired. The ops are already integrated in the merged store by flush.
    let hashes_for_new_db = this_cell_actions_op_basis_hashes.clone();
    authored_ops_to_dht_db_without_check(
        this_cell_actions_op_basis_hashes,
        authored_db.into(),
        dht_db,
    )
    .await?;

    space
        .dht_store
        .clear_op_withhold_publishes(hashes_for_new_db)
        .await
        .map_err(WorkflowError::from)?;

    integration_trigger.trigger(&"integrate countersigning_success");

    Ok(())
}

/// When the workflow has attempted to resolve a countersigning session but wasn't able to find a deterministic answer by querying peer state,
/// the session becomes unresolved and can be forcefully completed and published anyway.
pub(super) async fn force_publish_countersigning_session(
    space: Space,
    network: DynHolochainP2pDna,
    keystore: MetaLairClient,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
    cell_id: CellId,
    preflight_request: PreflightRequest,
) -> WorkflowResult<bool> {
    // Read the chain lock from the merged store (#5370). It isn't necessarily for
    // the current session; we can't check that until we have the session data.
    let chain_lock = space
        .dht_store
        .as_read()
        .get_chain_lock(cell_id.agent_pubkey().clone())
        .await?;

    // Read the current countersigning session from the merged store (#5370).
    let maybe_current_session = space
        .dht_store
        .as_read()
        .current_countersigning_session(cell_id.agent_pubkey())
        .await?;

    let maybe_session_record = match maybe_current_session {
        Some((session_record, _, session_data)) => {
            let lock_subject = session_data.preflight_request.fingerprint()?;
            if lock_subject != preflight_request.fingerprint()? {
                None
            } else {
                match &chain_lock {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    Some(chain_lock) if chain_lock.subject() == lock_subject => {
                        Some(session_record)
                    }
                    _ => None,
                }
            }
        }
        None => None,
    };

    let session_record = match maybe_session_record {
        Some(cs) => cs,
        None => {
            // If there is no active session then we can short circuit.
            tracing::info!("Received a signature bundle for a session that exists in state but is missing from the database");
            return Ok(false);
        }
    };

    let this_cells_action_hash = session_record.action_hashed().hash.clone();

    reveal_countersigning_session(
        space,
        network,
        keystore,
        session_record,
        cell_id.agent_pubkey(),
        this_cells_action_hash,
        integration_trigger,
        publish_trigger,
    )
    .await?;

    Ok(true)
}
