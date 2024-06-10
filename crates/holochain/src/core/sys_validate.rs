//! # System Validation Checks
//! This module contains all the checks we run for sys validation

use super::queue_consumer::TriggerSender;
use super::workflow::incoming_dht_ops_workflow::incoming_dht_ops_workflow;
use super::workflow::sys_validation_workflow::SysValidationWorkspace;
use crate::conductor::space::Space;
use holochain_keystore::AgentPubKeyExt;
use holochain_types::prelude::*;
use std::sync::Arc;

pub use error::*;
pub use holo_hash::*;
pub use holochain_state::source_chain::SourceChainError;
pub use holochain_state::source_chain::SourceChainResult;

#[allow(missing_docs)]
mod error;
#[cfg(test)]
mod tests;

/// 16mb limit on Entries due to websocket limits.
/// 4mb limit to constrain bandwidth usage on uploading.
/// (Assuming a baseline 5mbps upload for now... update this
/// as consumer internet connections trend toward more upload)
/// Consider splitting large entries up.
pub const MAX_ENTRY_SIZE: usize = ENTRY_SIZE_LIMIT;

/// 1kb limit on LinkTags.
/// Tags are used as keys to the database to allow
/// fast lookup so they should be small.
pub const MAX_TAG_SIZE: usize = 1000;

/// Verify the signature for this action
pub async fn verify_action_signature(sig: &Signature, action: &Action) -> SysValidationResult<()> {
    if action.author().verify_signature(sig, action).await? {
        Ok(())
    } else {
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::CounterfeitAction((*sig).clone(), (*action).clone()),
        ))
    }
}

/// Verify the signature for this warrant
pub async fn verify_warrant_signature(warrant_op: &WarrantOp) -> SysValidationResult<()> {
    let WarrantOp {
        author,
        signature,
        warrant,
        timestamp,
    } = warrant_op;
    if author
        .verify_signature(signature, &TimedWarrant(warrant.clone(), *timestamp))
        .await?
    {
        Ok(())
    } else {
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::CounterfeitWarrant(warrant_op.clone()),
        ))
    }
}

/// Verify the author key was valid at the time
/// of signing with dpki
/// TODO: This is just a stub until we have dpki.
pub async fn author_key_is_valid(_author: &AgentPubKey) -> SysValidationResult<()> {
    Ok(())
}

/// Verify the countersigning session contains the specified action.
pub fn check_countersigning_session_data_contains_action(
    entry_hash: EntryHash,
    session_data: &CounterSigningSessionData,
    action: NewEntryActionRef<'_>,
) -> SysValidationResult<()> {
    let weight = match action {
        NewEntryActionRef::Create(h) => h.weight.clone(),
        NewEntryActionRef::Update(h) => h.weight.clone(),
    };
    let action_is_in_session = session_data
        .build_action_set(entry_hash, weight)
        .map_err(SysValidationError::from)?
        .iter()
        .any(|session_action| match (&action, session_action) {
            (NewEntryActionRef::Create(create), Action::Create(session_create)) => {
                create == &session_create
            }
            (NewEntryActionRef::Update(update), Action::Update(session_update)) => {
                update == &session_update
            }
            _ => false,
        });
    if !action_is_in_session {
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::ActionNotInCounterSigningSession(
                session_data.to_owned(),
                action.to_new_entry_action(),
            ),
        ))
    } else {
        Ok(())
    }
}

/// Verify that the signature on a preflight request is valid.
pub async fn check_countersigning_preflight_response_signature(
    preflight_response: &PreflightResponse,
) -> SysValidationResult<()> {
    let signature_is_valid = preflight_response
        .request()
        .signing_agents
        .get(*preflight_response.agent_state().agent_index() as usize)
        .ok_or_else(|| {
            SysValidationError::ValidationOutcome(ValidationOutcome::PreflightResponseSignature(
                (*preflight_response).clone(),
            ))
        })?
        .0
        .verify_signature_raw(
            preflight_response.signature(),
            preflight_response
                .encode_for_signature()
                .map_err(|_| {
                    SysValidationError::ValidationOutcome(
                        ValidationOutcome::PreflightResponseSignature(
                            (*preflight_response).clone(),
                        ),
                    )
                })?
                .into(),
        )
        .await?;
    if signature_is_valid {
        Ok(())
    } else {
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::PreflightResponseSignature((*preflight_response).clone()),
        ))
    }
}

/// Verify all the countersigning session data together.
pub async fn check_countersigning_session_data(
    entry_hash: EntryHash,
    session_data: &CounterSigningSessionData,
    action: NewEntryActionRef<'_>,
) -> SysValidationResult<()> {
    session_data.check_integrity()?;
    check_countersigning_session_data_contains_action(entry_hash, session_data, action)?;

    let tasks: Vec<_> = session_data
        .responses()
        .iter()
        .map(|(response, signature)| async move {
            let preflight_response = PreflightResponse::try_new(
                session_data.preflight_request().clone(),
                response.clone(),
                signature.clone(),
            )?;
            check_countersigning_preflight_response_signature(&preflight_response).await
        })
        .collect();

    let results: Vec<SysValidationResult<()>> = futures::future::join_all(tasks).await;
    let results: SysValidationResult<()> = results.into_iter().collect();
    match results {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Check that the correct actions have the correct setting for prev_action:
/// - Dna can never have a prev_action, and must have seq == 0.
/// - All other actions must have prev_action, and seq > 0.
pub fn check_prev_action(action: &Action) -> SysValidationResult<()> {
    let is_dna = matches!(action, Action::Dna(_));
    let has_prev = action.prev_action().is_some();
    let is_first = action.action_seq() == 0;
    #[allow(clippy::collapsible_else_if)]
    if is_first {
        if is_dna && !has_prev {
            Ok(())
        } else {
            // Note that the implementation of the action types and `prev_action` should prevent this being hit
            // but this is useful as a defensive check.
            Err(PrevActionErrorKind::InvalidRoot)
        }
    } else {
        if !is_dna && has_prev {
            Ok(())
        } else {
            Err(PrevActionErrorKind::MissingPrev)
        }
    }
    .map_err(|e| ValidationOutcome::PrevActionError((e, action.clone()).into()).into())
}

/// Check that Dna actions are only added to empty source chains
pub fn check_valid_if_dna(action: &Action, dna_def: &DnaDefHashed) -> SysValidationResult<()> {
    match action {
        Action::Dna(a) => {
            let dna_hash = dna_def.as_hash();
            if a.hash != *dna_hash {
                Err(ValidationOutcome::WrongDna(a.hash.clone(), dna_hash.clone()).into())
            } else if action.timestamp() < dna_def.modifiers.origin_time {
                // If the Dna timestamp is ahead of the origin time, every other action
                // will be inductively so also due to the prev_action check
                Err(PrevActionErrorKind::InvalidRootOriginTime).map_err(|e| {
                    ValidationOutcome::PrevActionError((e, action.clone()).into()).into()
                })
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }
}

/// Check if there are other actions at this
/// sequence number
pub async fn check_chain_rollback(
    action: &Action,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<()> {
    let empty = workspace.action_seq_is_empty(action).await?;

    // Ok or log warning
    if empty {
        Ok(())
    } else {
        // TODO: implement real rollback detection once we know what that looks like
        tracing::error!(
            "Chain rollback detected at position {} for agent {:?} from action {:?}",
            action.action_seq(),
            action.author(),
            action,
        );
        Ok(())
    }
}

/// Placeholder for future spam check.
/// Check action timestamps don't exceed MAX_PUBLISH_FREQUENCY
pub async fn check_spam(_action: &Action) -> SysValidationResult<()> {
    Ok(())
}

/// Check that created agents are always paired with an AgentValidationPkg and vice versa
pub fn check_agent_validation_pkg_predecessor(
    action: &Action,
    prev_action: &Action,
) -> SysValidationResult<()> {
    let maybe_error = match (prev_action, action) {
        (
            Action::AgentValidationPkg(AgentValidationPkg { .. }),
            Action::Create(Create {
                entry_type: EntryType::AgentPubKey,
                ..
            })
            | Action::Update(Update {
                entry_type: EntryType::AgentPubKey,
                ..
            }),
        ) => None,
        (Action::AgentValidationPkg(AgentValidationPkg { .. }), _) => Some(
            "Every AgentValidationPkg must be followed by a Create or Update for an AgentPubKey",
        ),
        (
            _,
            Action::Create(Create {
                entry_type: EntryType::AgentPubKey,
                ..
            })
            | Action::Update(Update {
                entry_type: EntryType::AgentPubKey,
                ..
            }),
        ) => Some(
            "Every Create or Update for an AgentPubKey must be preceded by an AgentValidationPkg",
        ),
        _ => None,
    };

    if let Some(error) = maybe_error {
        Err(PrevActionErrorKind::InvalidSuccessor(
            error.to_string(),
            Box::new((prev_action.clone(), action.clone())),
        ))
        .map_err(|e| ValidationOutcome::PrevActionError((e, action.clone()).into()).into())
    } else {
        Ok(())
    }
}

/// Check that the author didn't change between actions
pub fn check_prev_author(action: &Action, prev_action: &Action) -> SysValidationResult<()> {
    // Agent updates will be valid when DPKI support lands
    let a1: AgentPubKey = if let Action::Update(
        u @ Update {
            entry_type: EntryType::AgentPubKey,
            ..
        },
    ) = prev_action
    {
        #[cfg(feature = "dpki")]
        {
            u.entry_hash.clone().into()
        }

        #[cfg(not(feature = "dpki"))]
        {
            u.author.clone()
        }
    } else {
        prev_action.author().clone()
    };

    let a2 = action.author();
    if a1 == *a2 {
        Ok(())
    } else {
        Err(PrevActionErrorKind::Author(a1, a2.clone()))
            .map_err(|e| ValidationOutcome::PrevActionError((e, action.clone()).into()).into())
    }
}

/// Check previous action timestamp is before this action
pub fn check_prev_timestamp(action: &Action, prev_action: &Action) -> SysValidationResult<()> {
    let t1 = prev_action.timestamp();
    let t2 = action.timestamp();
    if t2 >= t1 {
        Ok(())
    } else {
        Err(PrevActionErrorKind::Timestamp(t1, t2))
            .map_err(|e| ValidationOutcome::PrevActionError((e, action.clone()).into()).into())
    }
}

/// Check the previous action is one less than the current
pub fn check_prev_seq(action: &Action, prev_action: &Action) -> SysValidationResult<()> {
    let action_seq = action.action_seq();
    let prev_seq = prev_action.action_seq();
    if action_seq > 0 && prev_seq == action_seq - 1 {
        Ok(())
    } else {
        Err(PrevActionErrorKind::InvalidSeq(action_seq, prev_seq))
            .map_err(|e| ValidationOutcome::PrevActionError((e, action.clone()).into()).into())
    }
}

/// Check the entry variant matches the variant in the actions entry type
pub fn check_entry_type(entry_type: &EntryType, entry: &Entry) -> SysValidationResult<()> {
    entry_type_matches(entry_type, entry)
        .then_some(())
        .ok_or_else(|| ValidationOutcome::EntryTypeMismatch.into())
}

/// Check that the EntryVisibility is congruous with the presence or absence of entry data
pub fn check_entry_visibility(op: &ChainOp) -> SysValidationResult<()> {
    use EntryVisibility::*;
    use RecordEntry::*;

    let err = |reason: &str| {
        Err(ValidationOutcome::MalformedDhtOp(
            Box::new(op.action()),
            op.get_type(),
            reason.to_string(),
        )
        .into())
    };

    match (op.action().entry_type().map(|t| t.visibility()), op.entry()) {
        (Some(Public), Present(_)) => Ok(()),
        (Some(Private), Hidden) => Ok(()),
        (Some(Private), NotStored) => Ok(()),

        (Some(Public), Hidden) => err("RecordEntry::Hidden is only for Private entry type"),
        (Some(_), NA) => err("There is action entry data but the entry itself is N/A"),
        (Some(Private), Present(_)) => Err(ValidationOutcome::PrivateEntryLeaked.into()),
        (Some(Public), NotStored) => {
            if op.get_type() == ChainOpType::RegisterAgentActivity
                || op.action().entry_type() == Some(&EntryType::AgentPubKey)
            {
                // RegisterAgentActivity is a special case, where the entry data can be omitted.
                // Agent entries are also a special case. The "entry data" is already present in
                // the action as the entry hash, so no external entry data is needed.
                Ok(())
            } else {
                err("Op has public entry type but is missing its data")
            }
        }
        (None, NA) => Ok(()),
        (None, _) => err("Entry must be N/A for action with no entry type"),
    }
}

/// Check the actions entry hash matches the hash of the entry
pub fn check_entry_hash(hash: &EntryHash, entry: &Entry) -> SysValidationResult<()> {
    if *hash == EntryHash::with_data_sync(entry) {
        Ok(())
    } else {
        Err(ValidationOutcome::EntryHash.into())
    }
}

/// Check the action should have an entry.
/// Is either a Create or Update
pub fn check_new_entry_action(action: &Action) -> SysValidationResult<()> {
    match action {
        Action::Create(_) | Action::Update(_) => Ok(()),
        _ => Err(ValidationOutcome::NotNewEntry(action.clone()).into()),
    }
}

/// Check the entry size is under the MAX_ENTRY_SIZE
pub fn check_entry_size(entry: &Entry) -> SysValidationResult<()> {
    match entry {
        Entry::App(bytes) | Entry::CounterSign(_, bytes) => {
            let size = std::mem::size_of_val(&bytes.bytes()[..]);
            if size <= MAX_ENTRY_SIZE {
                Ok(())
            } else {
                Err(ValidationOutcome::EntryTooLarge(size).into())
            }
        }
        _ => {
            // TODO: size checks on other types (cap grant and claim)
            Ok(())
        }
    }
}

/// Check the link tag size is under the MAX_TAG_SIZE
pub fn check_tag_size(tag: &LinkTag) -> SysValidationResult<()> {
    let size = std::mem::size_of_val(&tag.0[..]);
    if size <= MAX_TAG_SIZE {
        Ok(())
    } else {
        Err(ValidationOutcome::TagTooLarge(size).into())
    }
}

/// Check a Update's entry type is the same for
/// original and new entry.
pub fn check_update_reference(
    update: &Update,
    original_entry_action: &NewEntryActionRef<'_>,
) -> SysValidationResult<()> {
    if update.entry_type != *original_entry_action.entry_type() {
        return Err(ValidationOutcome::UpdateTypeMismatch(
            original_entry_action.entry_type().clone(),
            update.entry_type.clone(),
        )
        .into());
    }

    if update.original_entry_address != *original_entry_action.entry_hash() {
        return Err(ValidationOutcome::UpdateHashMismatch(
            original_entry_action.entry_hash().clone(),
            update.original_entry_address.clone(),
        )
        .into());
    }

    Ok(())
}

/// Validate a chain of actions with an optional starting point.
pub fn validate_chain<'iter, A: 'iter + ChainItem>(
    mut actions: impl Iterator<Item = &'iter A>,
    persisted_chain_head: &Option<(A::Hash, u32)>,
) -> SysValidationResult<()> {
    // Check the chain starts in a valid way.
    let mut last_item = match actions.next() {
        Some(item) => {
            match persisted_chain_head {
                Some((prev_hash, prev_seq)) => {
                    check_prev_action_chain(prev_hash, *prev_seq, item)
                        .map_err(ValidationOutcome::from)?;
                }
                None => {
                    // If there's no persisted chain head, then the first action
                    // must have no parent.
                    if item.prev_hash().is_some() {
                        return Err(ValidationOutcome::PrevActionError(
                            (PrevActionErrorKind::InvalidRoot, item).into(),
                        )
                        .into());
                    }
                }
            }
            (item.get_hash(), item.seq())
        }
        None => return Ok(()),
    };

    for item in actions {
        // Check each item of the chain is valid.
        check_prev_action_chain(last_item.0, last_item.1, item).map_err(ValidationOutcome::from)?;
        last_item = (item.get_hash(), item.seq());
    }
    Ok(())
}

// Check the action is valid for the previous action.
fn check_prev_action_chain<A: ChainItem>(
    prev_action_hash: &A::Hash,
    prev_action_seq: u32,
    action: &A,
) -> Result<(), PrevActionError> {
    // The root cannot appear later in the chain
    if action.prev_hash().is_none() {
        Err((PrevActionErrorKind::MissingPrev, action).into())
    } else if action.prev_hash().map_or(true, |p| p != prev_action_hash) {
        // Check the prev hash matches.
        Err((PrevActionErrorKind::HashMismatch(action.seq()), action).into())
    } else if action
        .seq()
        .checked_sub(1)
        .map_or(true, |s| prev_action_seq != s)
    {
        // Check the prev seq is one less.
        Err((
            PrevActionErrorKind::InvalidSeq(action.seq(), prev_action_seq),
            action,
        )
            .into())
    } else {
        Ok(())
    }
}

/// Allows DhtOps to be sent to some receiver
#[async_trait::async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait DhtOpSender {
    /// Sends an op
    async fn send_op(&self, op: DhtOp) -> SysValidationResult<()>;

    /// Send a StoreRecord DhtOp
    async fn send_store_record(&self, record: Record) -> SysValidationResult<()>;

    /// Send a StoreEntry DhtOp
    async fn send_store_entry(&self, record: Record) -> SysValidationResult<()>;

    /// Send a RegisterAddLink DhtOp
    async fn send_register_add_link(&self, record: Record) -> SysValidationResult<()>;

    /// Send a RegisterAgentActivity DhtOp
    async fn send_register_agent_activity(&self, record: Record) -> SysValidationResult<()>;
}

/// Allows you to send an op to the
/// incoming_dht_ops_workflow if you
/// found it on the network and were supposed
/// to be holding it.
#[derive(derive_more::Constructor, Clone)]
pub struct IncomingDhtOpSender {
    space: Arc<Space>,
    sys_validation_trigger: TriggerSender,
}

#[async_trait::async_trait]
impl DhtOpSender for IncomingDhtOpSender {
    async fn send_op(&self, op: DhtOp) -> SysValidationResult<()> {
        let ops = vec![op];
        Ok(incoming_dht_ops_workflow(
            self.space.as_ref().clone(),
            self.sys_validation_trigger.clone(),
            ops,
            false,
        )
        .await
        .map_err(Box::new)?)
    }

    async fn send_store_record(&self, record: Record) -> SysValidationResult<()> {
        self.send_op(make_store_record(record).into()).await
    }

    async fn send_store_entry(&self, record: Record) -> SysValidationResult<()> {
        // TODO: MD: isn't it already too late if we've received a private entry from the network at this point?
        let is_public_entry = record.action().entry_type().map_or(false, |et| {
            matches!(et.visibility(), EntryVisibility::Public)
        });
        if is_public_entry {
            if let Some(op) = make_store_entry(record) {
                self.send_op(op.into()).await?;
            }
        }
        Ok(())
    }

    async fn send_register_add_link(&self, record: Record) -> SysValidationResult<()> {
        if let Some(op) = make_register_add_link(record) {
            self.send_op(op.into()).await?;
        }

        Ok(())
    }

    async fn send_register_agent_activity(&self, record: Record) -> SysValidationResult<()> {
        self.send_op(make_register_agent_activity(record).into())
            .await
    }
}

/// Make a StoreRecord ChainOp from a Record.
/// Note that this can fail if the op is missing an
/// Entry when it was supposed to have one.
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_store_record(record: Record) -> ChainOp {
    // Extract the data
    let (shh, record_entry) = record.privatized().0.into_inner();
    let (action, signature) = shh.into_inner();
    let action = action.into_content();

    // Create the op
    ChainOp::StoreRecord(signature, action, record_entry)
}

/// Make a StoreEntry ChainOp from a Record.
/// Note that this can fail if the op is missing an Entry or
/// the action is the wrong type.
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_store_entry(record: Record) -> Option<ChainOp> {
    // Extract the data
    let (shh, record_entry) = record.into_inner();
    let (action, signature) = shh.into_inner();

    // Check the entry and exit early if it's not there
    let entry_box = record_entry.into_option()?;
    // If the action is the wrong type exit early
    let action = action.into_content().try_into().ok()?;

    // Create the op
    let op = ChainOp::StoreEntry(signature, action, entry_box);
    Some(op)
}

/// Make a RegisterAddLink ChainOp from a Record.
/// Note that this can fail if the action is the wrong type
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_register_add_link(record: Record) -> Option<ChainOp> {
    // Extract the data
    let (shh, _) = record.into_inner();
    let (action, signature) = shh.into_inner();

    // If the action is the wrong type exit early
    let action = action.into_content().try_into().ok()?;

    // Create the op
    let op = ChainOp::RegisterAddLink(signature, action);
    Some(op)
}

/// Make a RegisterAgentActivity ChainOp from a Record.
/// Note that this can fail if the action is the wrong type
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_register_agent_activity(record: Record) -> ChainOp {
    // Extract the data
    let (shh, _) = record.into_inner();
    let (action, signature) = shh.into_inner();

    // TODO something seems to have changed here, should this not be able to fail?
    // If the action is the wrong type exit early
    let action = action.into_content();

    // Create the op
    ChainOp::RegisterAgentActivity(signature, action)
}

#[cfg(test)]
pub mod test {
    use super::check_countersigning_preflight_response_signature;
    use crate::core::sys_validate::error::SysValidationError;
    use crate::core::ValidationOutcome;
    use arbitrary::Arbitrary;
    use fixt::fixt;
    use fixt::Predictable;
    use hdk::prelude::AgentPubKeyFixturator;
    use holochain_keystore::AgentPubKeyExt;
    use holochain_zome_types::countersigning::PreflightResponse;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_check_countersigning_preflight_response_signature() {
        let keystore = holochain_keystore::test_keystore();
        let mut u = arbitrary::Unstructured::new(&[0; 1000]);
        let mut preflight_response = PreflightResponse::arbitrary(&mut u).unwrap();
        assert_matches!(
            check_countersigning_preflight_response_signature(&preflight_response).await,
            Err(SysValidationError::ValidationOutcome(
                ValidationOutcome::PreflightResponseSignature(_)
            ))
        );

        let alice = fixt!(AgentPubKey, Predictable);
        let bob = fixt!(AgentPubKey, Predictable, 1);

        preflight_response
            .request_mut()
            .signing_agents
            .push((alice.clone(), vec![]));
        preflight_response
            .request_mut()
            .signing_agents
            .push((bob, vec![]));

        *preflight_response.signature_mut() = alice
            .sign_raw(
                &keystore,
                preflight_response.encode_for_signature().unwrap().into(),
            )
            .await
            .unwrap();

        check_countersigning_preflight_response_signature(&preflight_response)
            .await
            .unwrap();
    }
}
