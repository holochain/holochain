//! Derives chain ops, `CapGrant` aux-table parameters, and serialized op sizes for a batch of
//! authored actions being written to the [`super::DhtStore`].

use holo_hash::{ActionHash, DhtOpHash, HasHash};
use holochain_types::dht_op::{
    produce_op_lites_from_iter, ChainOp, ChainOpLite, ChainOpUniqueForm, DhtOp, DhtOpLite, OpOrder,
};
use holochain_types::EntryHashed;
use holochain_zome_types::prelude::{
    Action, ActionHashed, CapAccess, Entry, EntryType, SignedAction, SignedActionHashed, Timestamp,
};

use crate::mutations::StateMutationResult;

/// Produces the chain ops for a batch of authored actions.
///
/// For each action, computes its [`ChainOpLite`]s via [`produce_op_lites_from_iter`] and each op's
/// [`DhtOpHash`] via [`ChainOpUniqueForm::op_hash`]. Returns the actions unchanged alongside the
/// per-op `(op, op_hash, op_order, timestamp, sys_validation_dependencies)` tuples.
#[allow(clippy::complexity)]
pub(crate) fn build_ops_from_actions(
    actions: Vec<SignedActionHashed>,
) -> StateMutationResult<(
    Vec<SignedActionHashed>,
    Vec<(DhtOpLite, DhtOpHash, OpOrder, Timestamp, Vec<ActionHash>)>,
)> {
    // Actions end up back in here.
    let mut actions_output = Vec::with_capacity(actions.len());
    // The op related data ends up here.
    let mut ops = Vec::with_capacity(actions.len());

    // Loop through each action and produce op related data.
    for shh in actions {
        // &ActionHash, &Action, EntryHash are needed to produce the ops.
        let entry_hash = shh.action().entry_hash().cloned();
        let item = (shh.as_hash(), shh.action(), entry_hash);
        let ops_inner = produce_op_lites_from_iter(vec![item].into_iter())?;

        // Break apart the SignedActionHashed.
        let (action, sig) = shh.into_inner();
        let (action, hash) = action.into_inner();

        // We need to take the action by value and put it back each loop.
        let mut h = Some(action);
        for op in ops_inner {
            let op_type = op.get_type();
            let op = DhtOpLite::from(op);
            // Action is required by value to produce the DhtOpHash.
            let (action, op_hash) =
                ChainOpUniqueForm::op_hash(op_type, h.expect("This can't be empty"))?;
            let op_order = OpOrder::new(op_type, action.timestamp());
            let timestamp = action.timestamp();
            // Put the action back by value.
            let deps = op_type.sys_validation_dependencies(&action);
            h = Some(action);
            // Collect the DhtOpLite, DhtOpHash and OpOrder.
            ops.push((op, op_hash, op_order, timestamp, deps));
        }

        // Put the SignedActionHashed back together.
        let shh = SignedActionHashed::with_presigned(
            ActionHashed::with_pre_hashed(h.expect("This can't be empty"), hash),
            sig,
        );
        // Put the action back in the list.
        actions_output.push(shh);
    }
    Ok((actions_output, ops))
}

/// Returns the encoded `cap_access` and optional `tag`, if the given action creates or updates an
/// [`Entry::CapGrant`]. Returns `None` for all other action types.
///
/// The entry content is needed to extract the tag; entries are looked up by the entry hash carried
/// by the action.
pub(crate) fn cap_grant_index_params(
    shh: &SignedActionHashed,
    entries: &[EntryHashed],
) -> Option<(i64, Option<String>)> {
    let (entry_type, entry_hash) = match shh.action() {
        Action::Create(d) => (&d.entry_type, &d.entry_hash),
        Action::Update(d) => (&d.entry_type, &d.entry_hash),
        _ => return None,
    };

    if !matches!(entry_type, EntryType::CapGrant) {
        return None;
    }

    // Find the matching entry in the batch.
    let entry = entries
        .iter()
        .find(|e| e.as_hash() == entry_hash)?
        .as_content();

    let cap_grant = match entry {
        Entry::CapGrant(g) => g,
        _ => return None,
    };

    let cap_access_i64 = match &cap_grant.access {
        CapAccess::Unrestricted => 0_i64,
        CapAccess::Transferable { .. } => 1_i64,
        CapAccess::Assigned { .. } => 2_i64,
    };
    // Deliberate empty→NULL normalisation: the schema stores an absent tag as NULL rather than an
    // empty string.
    let tag = if cap_grant.tag.is_empty() {
        None
    } else {
        Some(cap_grant.tag.clone())
    };

    Some((cap_access_i64, tag))
}

/// Encodes the wire-form [`DhtOp`] for a [`ChainOpLite`] and returns its serialized length in
/// bytes. The action is looked up by hash in `actions`; the entry (if any) is looked up by
/// [`Action::entry_hash`] in `entries`. Returns `0` only if the op cannot be reconstructed because
/// the action is missing, which would indicate a programming error in the caller.
pub(crate) fn encoded_chain_op_size(
    op: &ChainOpLite,
    actions: &[SignedActionHashed],
    entries: &[EntryHashed],
) -> u32 {
    let action_hash = op.action_hash();
    let Some(sah) = actions.iter().find(|sah| sah.as_hash() == action_hash) else {
        return 0;
    };
    let signed_action: SignedAction = (sah.action().clone(), sah.signature().clone()).into();
    let maybe_entry: Option<Entry> = signed_action
        .action()
        .entry_hash()
        .and_then(|eh| entries.iter().find(|e| e.as_hash() == eh))
        .map(|e| e.as_content().clone());

    match ChainOp::from_type(op.get_type(), signed_action, maybe_entry) {
        Ok(chain_op) => holochain_serialized_bytes::encode(&DhtOp::from(chain_op))
            .map(|b| b.len() as u32)
            .unwrap_or(0),
        Err(_) => 0,
    }
}
