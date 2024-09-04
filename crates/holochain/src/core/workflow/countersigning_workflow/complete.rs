use crate::conductor::space::Space;
use crate::core::queue_consumer::TriggerSender;
use crate::core::ribosome::weigh_placeholder;
use crate::core::workflow::{WorkflowError, WorkflowResult};
use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, EntryHash};
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::HolochainP2pDnaT;
use holochain_sqlite::db::ReadAccess;
use holochain_sqlite::error::DatabaseResult;
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::mutations;
use holochain_state::prelude::{current_countersigning_session, SourceChainResult, Store};
use holochain_types::dht_op::ChainOp;
use holochain_zome_types::prelude::SignedAction;
use rusqlite::{named_params, Transaction};
use std::sync::Arc;

pub(crate) async fn inner_countersigning_session_complete(
    space: Space,
    network: Arc<impl HolochainP2pDnaT>,
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
            if let Some((_, cs_entry_hash, session_data)) =
                current_countersigning_session(&txn, Arc::new(author.clone()))?
            {
                let lock_subject = holo_hash::encode::blake2b_256(
                    &holochain_serialized_bytes::encode(&session_data.preflight_request())?,
                );

                let chain_lock = holochain_state::chain_lock::get_chain_lock(&txn, &author)?;
                if let Some(chain_lock) = chain_lock {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    if cs_entry_hash != entry_hash || chain_lock.subject() != lock_subject {
                        return SourceChainResult::Ok(None);
                    }

                    let transaction: holochain_state::prelude::Txn = (&txn).into();
                    if transaction.contains_entry(&entry_hash)? {
                        return Ok(Some(session_data));
                    }
                }
            }
            SourceChainResult::Ok(None)
        }
    };

    let session_data = match authored_db.read_async(reader_closure).await? {
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
            tracing::info!("Invalid signature found: {:?}", sa);
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
        tracing::info!(
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
        tracing::info!("Have the right number of actions");

        // Check all stored action hashes match an incoming action hash.
        if stored_actions.iter().all(|a| {
            let a = ActionHash::with_data_sync(a);
            incoming_actions.iter().any(|i| *i == a)
        }) {
            tracing::info!("All hashes are correct");
            // All checks have passed, proceed to update the session state.
            integrity_check_passed = true;
        }
    }

    if !integrity_check_passed {
        // If the integrity check fails then we can't proceed with this signature bundle.
        tracing::info!("Integrity check failed for countersigning session");
        return Ok(None);
    }

    apply_success_state_changes(space, &author, this_cells_action_hash, integration_trigger)
        .await?;

    // Publish other signers agent activity ops to their agent activity authorities.
    for sa in signed_actions {
        let (action, signature) = sa.into();
        if *action.author() == author {
            continue;
        }
        let op = ChainOp::RegisterAgentActivity(signature, action);
        let basis = op.dht_basis();
        if let Err(e) = network.publish_countersign(false, basis, op.into()).await {
            tracing::error!(
                "Failed to publish to other countersigners agent authorities because of: {:?}",
                e
            );
        }
    }

    publish_trigger.trigger(&"publish countersigning_success");

    tracing::info!("Countersigning session complete for agent: {:?}", author);

    Ok(Some(session_data.preflight_request.app_entry_hash))
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
