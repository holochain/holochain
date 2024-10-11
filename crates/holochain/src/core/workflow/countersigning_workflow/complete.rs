use crate::conductor::space::Space;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::{WorkflowError, WorkflowResult};
use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, EntryHash};
use holochain_chc::AddRecordPayload;
use holochain_keystore::{AgentPubKeyExt, MetaLairClient};
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::db::ReadAccess;
use holochain_sqlite::error::DatabaseResult;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::mutations;
use holochain_state::prelude::{
    current_countersigning_session, SourceChainError, SourceChainResult, Store,
};
use holochain_types::{
    dht_op::ChainOp,
    prelude::{CellId, PreflightRequest, Record},
};
use holochain_zome_types::prelude::SignedAction;
use kitsune_p2p_types::dht::prelude::Timestamp;
use rusqlite::{named_params, Transaction};
use std::sync::Arc;

pub(crate) async fn inner_countersigning_session_complete(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    keystore: MetaLairClient,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<Option<EntryHash>> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;

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

    // Do a quick check to see if this entry hash matches the current locked session, so we don't
    // check signatures unless there is an active session.
    let reader_closure = {
        let entry_hash = entry_hash.clone();
        let author = author.clone();
        move |txn: Transaction| {
            // This chain lock isn't necessarily for the current session, we can't check that until later.
            if let Some((session_record, cs_entry_hash, session_data)) =
                current_countersigning_session(&txn, Arc::new(author.clone()))?
            {
                let lock_subject = session_data.preflight_request.fingerprint()?;

                let chain_lock = holochain_state::chain_lock::get_chain_lock(&txn, &author)?;
                if let Some(chain_lock) = chain_lock {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    if cs_entry_hash != entry_hash || chain_lock.subject() != lock_subject {
                        return SourceChainResult::Ok(None);
                    }

                    let transaction: holochain_state::prelude::Txn = (&txn).into();
                    // Ensure that the entry is present in the database.
                    // We've looked up the session as a Record, but that permits the entry to be
                    // missing. The cs_entry_hash is stored on the action rather than being a
                    // guarantee that the action is present.
                    if transaction.contains_entry(&entry_hash)? {
                        return Ok(Some((session_record, session_data)));
                    }
                }
            }
            SourceChainResult::Ok(None)
        }
    };

    let (session_record, session_data) = match authored_db.read_async(reader_closure).await? {
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
            incoming_actions.iter().any(|i| *i == a)
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

    // TODO This should be in the publish workflow
    // Publish other signers agent activity ops to their agent activity authorities.
    for sa in signed_actions {
        let (action, signature) = sa.into();
        if *action.author() == author {
            continue;
        }
        let op = ChainOp::RegisterAgentActivity(signature, action);
        let basis = op.dht_basis();
        // TODO this is what flag is for, whether to witness or store - document and rename me
        if let Err(e) = network.publish_countersign(false, basis, op.into()).await {
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
    network: Arc<impl HolochainP2pDnaT>,
    keystore: MetaLairClient,
    session_record: Record,
    author: &AgentPubKey,
    this_cells_action_hash: ActionHash,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
) -> WorkflowResult<()> {
    if let Some(chc) = network.chc() {
        tracing::info!(
            "Adding countersigning session record to the CHC: {:?}",
            session_record
        );
        let payload =
            AddRecordPayload::from_records(keystore, author.clone(), vec![session_record])
                .await
                .map_err(SourceChainError::other)?;

        // TODO Need to be able to recover from this by pushing when we're behind the CHC.
        // This is a serious failure, but we have to continue with the workflow.
        // It would be worse to not publish the session record than to be out of sync with the CHC.
        // Being behind the CHC is a recoverable state, or should be at some point. We don't want
        // to try and figure out a partial publish of the session record from an unknown state.
        if let Err(e) = chc.add_records_request(payload).await {
            tracing::error!(
                "Failed to add countersigning session record to the CHC: {:?}",
                e
            );
        }
    }

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
    let dht_db_cache = space.dht_query_cache.clone();

    // Unlock the chain and remove the withhold publish flag from all ops in this session.
    let this_cell_actions_op_basis_hashes = authored_db
        .write_async({
            let author = author.clone();
            move |txn| -> SourceChainResult<Vec<DhtOpHash>> {
                // All checks have passed so unlock the chain.
                mutations::unlock_chain(txn, &author)?;
                // Update ops to publish.
                txn.execute(
                    "UPDATE DhtOp SET withhold_publish = NULL WHERE action_hash = :action_hash",
                    named_params! {
                        ":action_hash": this_cells_action_hash,
                    },
                )
                .map_err(holochain_state::prelude::StateMutationError::from)?;

                // Load the op hashes for this session so that we can publish them.
                Ok(get_countersigning_op_hashes(txn, this_cells_action_hash)?)
            }
        })
        .await?;

    // If all signatures are valid (above) and i signed then i must have
    // validated it previously so i now agree that i authored it.
    // TODO: perhaps this should be `authored_ops_to_dht_db`, i.e. the arc check should
    //       be performed, because we may not be an authority for these ops
    authored_ops_to_dht_db_without_check(
        this_cell_actions_op_basis_hashes,
        authored_db.into(),
        dht_db,
        &dht_db_cache,
    )
    .await?;

    integration_trigger.trigger(&"integrate countersigning_success");

    Ok(())
}

fn get_countersigning_op_hashes(
    txn: &mut Transaction,
    this_cells_action_hash: ActionHash,
) -> DatabaseResult<Vec<DhtOpHash>> {
    Ok(txn
        .prepare("SELECT basis_hash, hash FROM DhtOp WHERE action_hash = :action_hash")?
        .query_map(
            named_params! {
                ":action_hash": this_cells_action_hash
            },
            |row| {
                let hash: DhtOpHash = row.get("hash")?;
                Ok(hash)
            },
        )?
        .collect::<Result<Vec<_>, _>>()?)
}

/// When it has been attempted to resolve a countersigning session unsuccessfully by querying peer state,
/// the session becomes unresolved and can be forcefully completed and published anyway.
pub async fn force_publish_countersigning_session(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
    keystore: MetaLairClient,
    integration_trigger: TriggerSender,
    publish_trigger: TriggerSender,
    cell_id: CellId,
    preflight_request: PreflightRequest,
) -> WorkflowResult<bool> {
    // Query database for current countersigning session.
    let reader_closure = {
        let author = cell_id.agent_pubkey().clone();
        let preflight_request = preflight_request.clone();
        move |txn: Transaction| {
            // This chain lock isn't necessarily for the current session, we can't check that until later.
            if let Some((session_record, _, session_data)) =
                current_countersigning_session(&txn, Arc::new(author.clone()))?
            {
                let lock_subject = session_data.preflight_request.fingerprint()?;
                if lock_subject != preflight_request.fingerprint()? {
                    return SourceChainResult::Ok(None);
                }

                let chain_lock = holochain_state::chain_lock::get_chain_lock(&txn, &author)?;
                if let Some(chain_lock) = chain_lock {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    if chain_lock.subject() != lock_subject {
                        return SourceChainResult::Ok(None);
                    }

                    return Ok(Some(session_record));
                }
            }
            SourceChainResult::Ok(None)
        }
    };
    let authored_db = space.get_or_create_authored_db(cell_id.agent_pubkey().clone())?;
    let session_record = match authored_db.read_async(reader_closure).await? {
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
