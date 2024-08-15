//! Countersigning workflow to maintain countersigning session state.

use super::error::WorkflowResult;
use crate::conductor::space::Space;
use crate::core::queue_consumer::QueueTriggers;
use crate::core::ribosome::weigh_placeholder;
use holo_hash::{ActionHash, AgentPubKey, DhtOpHash, OpBasis};
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::event::CountersigningSessionNegotiationMessage;
use holochain_p2p::{HolochainP2pDna, HolochainP2pDnaT};
use holochain_state::integrate::authored_ops_to_dht_db_without_check;
use holochain_state::mutations;
use holochain_state::prelude::*;
use rusqlite::{named_params, Transaction};
use std::sync::Arc;
use tokio::sync::broadcast;

/// An incoming countersigning session success.
#[cfg_attr(feature = "instrument", tracing::instrument(skip_all))]
pub(crate) async fn countersigning_success(
    space: Space,
    network: &HolochainP2pDna,
    author: AgentPubKey,
    signed_actions: Vec<SignedAction>,
    trigger: QueueTriggers,
    signal: broadcast::Sender<Signal>,
) -> WorkflowResult<()> {
    let authored_db = space.get_or_create_authored_db(author.clone())?;
    let dht_db = space.dht_db;
    let dht_db_cache = space.dht_query_cache;
    let QueueTriggers {
        publish_dht_ops: publish_trigger,
        integrate_dht_ops: integration_trigger,
        ..
    } = trigger;
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
        None => return Ok(()),
    };

    // Do a quick check to see if this entry hash matches the current locked session so we don't
    // check signatures unless there is an active session.
    let reader_closure = {
        let entry_hash = entry_hash.clone();
        let this_cells_action_hash = this_cells_action_hash.clone();
        let author = author.clone();
        move |txn: Transaction| {
            // This chain lock isn't necessarily for the current session, we can't check that until later.
            if let Some((cs_entry_hash, session_data)) =
                current_countersigning_session(&txn, Arc::new(author.clone()))?
            {
                let lock_subject = holo_hash::encode::blake2b_256(
                    &holochain_serialized_bytes::encode(&session_data.preflight_request())?,
                );

                if let Some(subject) =
                    holochain_state::chain_lock::get_chain_lock_subject(&txn, &author)?
                {
                    // This is the case where we have already locked the chain for another session and are
                    // receiving another signature bundle from a different session. We don't need this, so
                    // it's safe to short circuit.
                    if cs_entry_hash != entry_hash || subject != lock_subject {
                        return SourceChainResult::Ok(None);
                    }

                    let transaction: holochain_state::prelude::Txn = (&txn).into();
                    if transaction.contains_entry(&entry_hash)? {
                        // If this is a countersigning session we can grab all the ops for this
                        // cells action, so we can check if we need to self-publish them.
                        let r = txn
                            .prepare(
                                "SELECT basis_hash, hash FROM DhtOp WHERE action_hash = :action_hash",
                            ).map_err(DatabaseError::from)?
                            .query_map(
                                named_params! {
                                ":action_hash": this_cells_action_hash
                            },
                                |row| {
                                    let hash: DhtOpHash = row.get("hash")?;
                                    let basis: OpBasis = row.get("basis_hash")?;
                                    Ok((hash, basis))
                                },
                            ).map_err(DatabaseError::from)?
                            .collect::<Result<Vec<_>, _>>().map_err(DatabaseError::from)?;
                        return Ok(Some((session_data, r)));
                    }
                }
            }
            SourceChainResult::Ok(None)
        }
    };

    let (session_data, this_cell_actions_op_basis_hashes) =
        match authored_db.read_async(reader_closure).await? {
            Some((cs, r)) => (cs, r),
            None => {
                // If there is no active session then we can short circuit.
                return Ok(());
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
            return Ok(());
        }
        if sa.action().author() == &author {
            i_am_an_author = true;
        }
    }
    // Countersigning success is ultimately between authors to agree and publish.
    if !i_am_an_author {
        return Ok(());
    }

    // Hash actions.
    let incoming_actions: Vec<_> = signed_actions
        .iter()
        .map(ActionHash::with_data_sync)
        .collect();

    let result = authored_db
        .write_async({
            let author = author.clone();
            let entry_hash = entry_hash.clone();
            move |txn| {
                let weight = weigh_placeholder();
                let stored_actions = session_data.build_action_set(entry_hash, weight)?;
                if stored_actions.len() == incoming_actions.len() {
                    // Check all stored action hashes match an incoming action hash.
                    if stored_actions.iter().all(|a| {
                        let a = ActionHash::with_data_sync(a);
                        incoming_actions.iter().any(|i| *i == a)
                    }) {
                        // All checks have passed so unlock the chain.
                        mutations::unlock_chain(txn, &author)?;
                        // Update ops to publish.
                        txn.execute("UPDATE DhtOp SET withhold_publish = NULL WHERE action_hash = :action_hash",
                        named_params! {
                            ":action_hash": this_cells_action_hash,
                            }
                        ).map_err(holochain_state::prelude::StateMutationError::from)?;
                        return Ok(Some(session_data));
                    }
                }
                SourceChainResult::Ok(None)
        }})
        .await?;

    if let Some(session_data) = result {
        // If all signatures are valid (above) and i signed then i must have
        // validated it previously so i now agree that i authored it.
        // TODO: perhaps this should be `authored_ops_to_dht_db`, i.e. the arc check should
        //       be performed, because we may not be an authority for these ops
        authored_ops_to_dht_db_without_check(
            this_cell_actions_op_basis_hashes
                .into_iter()
                .map(|(op_hash, _)| op_hash)
                .collect(),
            authored_db.into(),
            dht_db,
            &dht_db_cache,
        )
        .await?;
        integration_trigger.trigger(&"countersigning_success");
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

        // Signal to the UI.
        // If there are no active connections this won't emit anything.
        let app_entry_hash = session_data.preflight_request.app_entry_hash.clone();
        signal
            .send(Signal::System(SystemSignal::SuccessfulCountersigning(
                app_entry_hash,
            )))
            .ok();

        publish_trigger.trigger(&"publish countersigning_success");
    }
    Ok(())
}

/// Publish to entry authorities so they can gather all the signed
/// actions for this session and respond with a session complete.
pub async fn countersigning_publish(
    network: &HolochainP2pDna,
    op: ChainOp,
    _author: AgentPubKey,
) -> Result<(), ZomeCallResponse> {
    if let Some(enzyme) = op.enzymatic_countersigning_enzyme() {
        if let Err(e) = network
            .countersigning_session_negotiation(
                vec![enzyme.clone()],
                CountersigningSessionNegotiationMessage::EnzymePush(Box::new(op)),
            )
            .await
        {
            tracing::error!(
                "Failed to push countersigning ops to enzyme because of: {:?}",
                e
            );
            return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
        }
    } else {
        let basis = op.dht_basis();
        if let Err(e) = network.publish_countersign(true, basis, op.into()).await {
            tracing::error!(
                "Failed to publish to entry authorities for countersigning session because of: {:?}",
                e
            );
            return Err(ZomeCallResponse::CountersigningSession(e.to_string()));
        }
    }
    Ok(())
}
