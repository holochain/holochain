//! # Publish Dht Op Workflow
//!
//! ## Open questions
//! - [x] Publish add and remove links on private entries, what are the constraints on when to publish
//!
//! For now, Publish links on private entries
// TODO: B-01827 Make story about: later consider adding a flag to make a link private and not publish it.
//       Even for those private links, we may need to publish them to the author of the private entry
//       (and we'd have to reference its action  which actually exists on the DHT to make that work,
//       rather than the entry which does not exist on the DHT).
//!
//!

use super::error::WorkflowResult;
use crate::core::queue_consumer::TriggerSender;
use crate::core::queue_consumer::WorkComplete;
use holo_hash::*;
use holochain_p2p::DynHolochainP2pDna;
use holochain_state::prelude::*;
use std::collections::HashMap;
use std::time;
use std::time::Duration;
use tracing::*;

mod publish_query;
pub use publish_query::{get_ops_to_publish, num_still_needing_publish};

#[cfg(test)]
mod unit_tests;

/// Default redundancy factor for validation receipts
pub const DEFAULT_RECEIPT_BUNDLE_SIZE: u8 = 5;

#[cfg_attr(
    feature = "instrument",
    tracing::instrument(skip(db, network, trigger_self, min_publish_interval))
)]
pub async fn publish_dht_ops_workflow(
    db: DbWrite<DbKindAuthored>,
    network: DynHolochainP2pDna,
    trigger_self: TriggerSender,
    agent: AgentPubKey,
    min_publish_interval: Duration,
) -> WorkflowResult<WorkComplete> {
    let mut complete = WorkComplete::Complete;
    let to_publish =
        publish_dht_ops_workflow_inner(db.clone().into(), agent.clone(), min_publish_interval)
            .await?;
    let to_publish_count: usize = to_publish.values().map(Vec::len).sum();

    if to_publish_count > 0 {
        info!(?agent, "publishing {to_publish_count} ops");
    }

    // Commit to the network
    let mut success = Vec::with_capacity(to_publish.len());
    for (basis, list) in to_publish {
        let (op_hash_list, op_data_list): (Vec<_>, Vec<_>) = list.into_iter().unzip();
        match network
            .publish(
                basis,
                agent.clone(),
                op_hash_list.clone(),
                None,
                Some(op_data_list),
            )
            .await
        {
            Err(e) => {
                // If we get a routing error it means the space hasn't started yet and we should try publishing again.
                if let holochain_p2p::HolochainP2pError::RoutingDnaError(_) = e {
                    // TODO if this doesn't change what is the loop terminate condition?
                    complete = WorkComplete::Incomplete(None);
                }
                warn!(failed_to_send_publish = ?e);
            }
            Ok(()) => {
                success.extend(op_hash_list);
            }
        }
    }

    if to_publish_count > 0 {
        info!(
            ?agent,
            "published {}/{} ops",
            success.len(),
            to_publish_count
        );
    }

    let now = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?;
    let continue_publish = db
        .write_async({
            let agent = agent.clone();
            move |txn| {
                for hash in success {
                    set_last_publish_time(txn, &hash, now)?;
                }
                WorkflowResult::Ok(publish_query::num_still_needing_publish(txn, agent)? > 0)
            }
        })
        .await?;

    // If we have more ops that could be published then continue looping.
    if continue_publish {
        trigger_self.resume_loop();
    } else {
        trigger_self.pause_loop();
    }

    debug!(?agent, "committed published ops");

    // --- END OF WORKFLOW, BEGIN FINISHER BOILERPLATE ---

    Ok(complete)
}

/// Read the authored for ops with receipt count < R
pub async fn publish_dht_ops_workflow_inner(
    db: DbRead<DbKindAuthored>,
    agent: AgentPubKey,
    min_publish_interval: Duration,
) -> WorkflowResult<HashMap<OpBasis, Vec<(DhtOpHash, crate::prelude::DhtOp)>>> {
    // Ops to publish by basis
    let mut to_publish = HashMap::new();

    for (basis, op_hash, op) in get_ops_to_publish(agent, &db, min_publish_interval).await? {
        // For every op publish a request
        // Collect and sort ops by basis
        to_publish
            .entry(basis)
            .or_insert_with(Vec::new)
            .push((op_hash, op));
    }

    Ok(to_publish)
}
