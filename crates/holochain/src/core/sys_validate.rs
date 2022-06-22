//! # System Validation Checks
//! This module contains all the checks we run for sys validation

use super::queue_consumer::TriggerSender;
use super::ribosome::RibosomeT;
use super::workflow::incoming_dht_ops_workflow::incoming_dht_ops_workflow;
use super::workflow::sys_validation_workflow::SysValidationWorkspace;
use crate::conductor::entry_def_store::get_entry_def;
use crate::conductor::handle::ConductorHandleT;
use crate::conductor::space::Space;
use holochain_keystore::AgentPubKeyExt;
use holochain_p2p::HolochainP2pDna;
use holochain_types::prelude::*;
use holochain_zome_types::countersigning::CounterSigningSessionData;
use std::convert::TryInto;
use std::sync::Arc;

pub(super) use error::*;
pub use holo_hash::*;
pub use holochain_state::source_chain::SourceChainError;
pub use holochain_state::source_chain::SourceChainResult;
pub use holochain_zome_types::ActionHashed;
pub use holochain_zome_types::Timestamp;

#[allow(missing_docs)]
mod error;
#[cfg(test)]
mod tests;

/// 16mb limit on Entries due to websocket limits.
/// Consider splitting large entries up.
pub const MAX_ENTRY_SIZE: usize = 16_000_000;

/// 1kb limit on LinkTags.
/// Tags are used as keys to the database to allow
/// fast lookup so they should be small.
pub const MAX_TAG_SIZE: usize = 1000;

/// Verify the signature for this action
pub async fn verify_action_signature(sig: &Signature, action: &Action) -> SysValidationResult<()> {
    if action.author().verify_signature(sig, action).await {
        Ok(())
    } else {
        Err(SysValidationError::ValidationOutcome(
            ValidationOutcome::Counterfeit((*sig).clone(), (*action).clone()),
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
    let action_is_in_session = session_data
        .build_action_set(entry_hash)
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
        .await;
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

/// Check that previous action makes sense
/// for this action.
/// If not Dna then cannot be root of chain
/// and must have previous action
pub fn check_prev_action(action: &Action) -> SysValidationResult<()> {
    match &action {
        Action::Dna(_) => Ok(()),
        _ => {
            if action.action_seq() > 0 {
                action
                    .prev_action()
                    .ok_or(PrevActionError::MissingPrev)
                    .map_err(ValidationOutcome::from)?;
                Ok(())
            } else {
                Err(PrevActionError::InvalidRoot).map_err(|e| ValidationOutcome::from(e).into())
            }
        }
    }
}

/// Check that Dna actions are only added to empty source chains
pub async fn check_valid_if_dna(
    action: &Action,
    workspace: &SysValidationWorkspace,
) -> SysValidationResult<()> {
    match action {
        Action::Dna(_) => {
            if !workspace.is_chain_empty(action.author()).await? {
                Err(PrevActionError::InvalidRoot).map_err(|e| ValidationOutcome::from(e).into())
            } else if action.timestamp() < workspace.dna_def().origin_time {
                // If the Dna timestamp is ahead of the origin time, every other action
                // will be inductively so also due to the prev_action check
                Err(PrevActionError::InvalidRootOriginTime)
                    .map_err(|e| ValidationOutcome::from(e).into())
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

/// Check previous action timestamp is before this action
pub fn check_prev_timestamp(action: &Action, prev_action: &Action) -> SysValidationResult<()> {
    if action.timestamp() > prev_action.timestamp() {
        Ok(())
    } else {
        Err(PrevActionError::Timestamp).map_err(|e| ValidationOutcome::from(e).into())
    }
}

/// Check the previous action is one less than the current
pub fn check_prev_seq(action: &Action, prev_action: &Action) -> SysValidationResult<()> {
    let action_seq = action.action_seq();
    let prev_seq = prev_action.action_seq();
    if action_seq > 0 && prev_seq == action_seq - 1 {
        Ok(())
    } else {
        Err(PrevActionError::InvalidSeq(action_seq, prev_seq))
            .map_err(|e| ValidationOutcome::from(e).into())
    }
}

/// Check the entry variant matches the variant in the actions entry type
pub fn check_entry_type(entry_type: &EntryType, entry: &Entry) -> SysValidationResult<()> {
    match (entry_type, entry) {
        (EntryType::AgentPubKey, Entry::Agent(_)) => Ok(()),
        (EntryType::App(_), Entry::App(_)) => Ok(()),
        (EntryType::App(_), Entry::CounterSign(_, _)) => Ok(()),
        (EntryType::CapClaim, Entry::CapClaim(_)) => Ok(()),
        (EntryType::CapGrant, Entry::CapGrant(_)) => Ok(()),
        _ => Err(ValidationOutcome::EntryType.into()),
    }
}

/// Check the AppEntryType is valid for the zome.
/// Check the EntryDefId and ZomeId are in range.
pub async fn check_app_entry_type(
    dna_hash: &DnaHash,
    entry_type: &AppEntryType,
    conductor: &dyn ConductorHandleT,
) -> SysValidationResult<EntryDef> {
    // We want to be careful about holding locks open to the conductor api
    // so calls are made in blocks
    let ribosome = conductor
        .get_ribosome(dna_hash)
        .map_err(|_| SysValidationError::DnaMissing(dna_hash.clone()))?;

    // Check if the zome is found
    let zome = ribosome
        .find_zome_from_entry(&entry_type.id())
        .ok_or_else(|| ValidationOutcome::ZomeId(entry_type.clone()))?
        .into_inner()
        .1;

    let entry_def = get_entry_def(entry_type.id(), zome, dna_hash, conductor).await?;

    // Check the visibility and return
    match entry_def {
        Some(entry_def) => {
            if entry_def.visibility == *entry_type.visibility() {
                Ok(entry_def)
            } else {
                Err(ValidationOutcome::EntryVisibility(entry_type.clone()).into())
            }
        }
        None => Err(ValidationOutcome::EntryDefId(entry_type.clone()).into()),
    }
}

/// Check the app entry type isn't private for store entry
pub fn check_not_private(entry_def: &EntryDef) -> SysValidationResult<()> {
    match entry_def.visibility {
        EntryVisibility::Public => Ok(()),
        EntryVisibility::Private => Err(ValidationOutcome::PrivateEntry.into()),
    }
}

/// Check the actions entry hash matches the hash of the entry
pub async fn check_entry_hash(hash: &EntryHash, entry: &Entry) -> SysValidationResult<()> {
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
        Entry::App(bytes) => {
            let size = std::mem::size_of_val(&bytes.bytes()[..]);
            if size < MAX_ENTRY_SIZE {
                Ok(())
            } else {
                Err(ValidationOutcome::EntryTooLarge(size, MAX_ENTRY_SIZE).into())
            }
        }
        // Other entry types are small
        _ => Ok(()),
    }
}

/// Check the link tag size is under the MAX_TAG_SIZE
pub fn check_tag_size(tag: &LinkTag) -> SysValidationResult<()> {
    let size = std::mem::size_of_val(&tag.0[..]);
    if size < MAX_TAG_SIZE {
        Ok(())
    } else {
        Err(ValidationOutcome::TagTooLarge(size, MAX_TAG_SIZE).into())
    }
}

/// Check a Update's entry type is the same for
/// original and new entry.
pub fn check_update_reference(
    eu: &Update,
    original_entry_action: &NewEntryActionRef<'_>,
) -> SysValidationResult<()> {
    if eu.entry_type == *original_entry_action.entry_type() {
        Ok(())
    } else {
        Err(ValidationOutcome::UpdateTypeMismatch(
            eu.entry_type.clone(),
            original_entry_action.entry_type().clone(),
        )
        .into())
    }
}

/// Validate a chain of actions with an optional starting point.
pub fn validate_chain<'iter>(
    mut actions: impl Iterator<Item = &'iter ActionHashed>,
    persisted_chain_head: &Option<(ActionHash, u32)>,
) -> SysValidationResult<()> {
    // Check the chain starts in a valid way.
    let mut last_item = match actions.next() {
        Some(ActionHashed {
            hash,
            content: action,
        }) => {
            match persisted_chain_head {
                Some((prev_hash, prev_seq)) => {
                    check_prev_action_chain(prev_hash, *prev_seq, action)
                        .map_err(ValidationOutcome::from)?;
                }
                None => {
                    // If there's no persisted chain head, then the first action
                    // must be a DNA.
                    if !matches!(action, Action::Dna(_)) {
                        return Err(ValidationOutcome::from(PrevActionError::InvalidRoot).into());
                    }
                }
            }
            let seq = action.action_seq();
            (hash, seq)
        }
        None => return Ok(()),
    };

    for ActionHashed {
        hash,
        content: action,
    } in actions
    {
        // Check each item of the chain is valid.
        check_prev_action_chain(last_item.0, last_item.1, action)
            .map_err(ValidationOutcome::from)?;
        last_item = (hash, action.action_seq());
    }
    Ok(())
}

// Check the action is valid for the previous action.
fn check_prev_action_chain(
    prev_action_hash: &ActionHash,
    prev_action_seq: u32,
    action: &Action,
) -> Result<(), PrevActionError> {
    // DNA cannot appear later in the chain.
    if matches!(action, Action::Dna(_)) {
        Err(PrevActionError::InvalidRoot)
    } else if action.prev_action().map_or(true, |p| p != prev_action_hash) {
        // Check the prev hash matches.
        Err(PrevActionError::HashMismatch)
    } else if action
        .action_seq()
        .checked_sub(1)
        .map_or(true, |s| prev_action_seq != s)
    {
        // Check the prev seq is one less.
        Err(PrevActionError::InvalidSeq(
            action.action_seq(),
            prev_action_seq,
        ))
    } else {
        Ok(())
    }
}

/// If we are not holding this action then
/// retrieve it and send it as a RegisterAddLink DhtOp
/// to our incoming_dht_ops_workflow.
///
/// Apply a checks callback to the Record.
///
/// Additionally sys validation will be triggered to
/// run again if we weren't holding it.
pub async fn check_and_hold_register_add_link<F>(
    hash: &ActionHash,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
    f: F,
) -> SysValidationResult<()>
where
    F: FnOnce(&Record) -> SysValidationResult<()>,
{
    let source = check_and_hold(hash, workspace, network).await?;
    f(source.as_ref())?;
    if let (Some(incoming_dht_ops_sender), Source::Network(record)) =
        (incoming_dht_ops_sender, source)
    {
        incoming_dht_ops_sender
            .send_register_add_link(record)
            .await?;
    }
    Ok(())
}

/// If we are not holding this action then
/// retrieve it and send it as a RegisterAgentActivity DhtOp
/// to our incoming_dht_ops_workflow.
///
/// Apply a checks callback to the Record.
///
/// Additionally sys validation will be triggered to
/// run again if we weren't holding it.
pub async fn check_and_hold_register_agent_activity<F>(
    hash: &ActionHash,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
    f: F,
) -> SysValidationResult<()>
where
    F: FnOnce(&Record) -> SysValidationResult<()>,
{
    let source = check_and_hold(hash, workspace, network).await?;
    f(source.as_ref())?;
    if let (Some(incoming_dht_ops_sender), Source::Network(record)) =
        (incoming_dht_ops_sender, source)
    {
        incoming_dht_ops_sender
            .send_register_agent_activity(record)
            .await?;
    }
    Ok(())
}

/// If we are not holding this action then
/// retrieve it and send it as a StoreEntry DhtOp
/// to our incoming_dht_ops_workflow.
///
/// Apply a checks callback to the Record.
///
/// Additionally sys validation will be triggered to
/// run again if we weren't holding it.
pub async fn check_and_hold_store_entry<F>(
    hash: &ActionHash,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
    f: F,
) -> SysValidationResult<()>
where
    F: FnOnce(&Record) -> SysValidationResult<()>,
{
    let source = check_and_hold(hash, workspace, network).await?;
    f(source.as_ref())?;
    if let (Some(incoming_dht_ops_sender), Source::Network(record)) =
        (incoming_dht_ops_sender, source)
    {
        incoming_dht_ops_sender.send_store_entry(record).await?;
    }
    Ok(())
}

/// If we are not holding this entry then
/// retrieve any record at this EntryHash
/// and send it as a StoreEntry DhtOp
/// to our incoming_dht_ops_workflow.
///
/// Note this is different to check_and_hold_store_entry
/// because it gets the Record via an EntryHash which
/// means it will be any Record.
///
/// Apply a checks callback to the Record.
///
/// Additionally sys validation will be triggered to
/// run again if we weren't holding it.
pub async fn check_and_hold_any_store_entry<F>(
    hash: &EntryHash,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
    f: F,
) -> SysValidationResult<()>
where
    F: FnOnce(&Record) -> SysValidationResult<()>,
{
    let source = check_and_hold(hash, workspace, network).await?;
    f(source.as_ref())?;
    if let (Some(incoming_dht_ops_sender), Source::Network(record)) =
        (incoming_dht_ops_sender, source)
    {
        incoming_dht_ops_sender.send_store_entry(record).await?;
    }
    Ok(())
}

/// If we are not holding this action then
/// retrieve it and send it as a StoreRecord DhtOp
/// to our incoming_dht_ops_workflow.
///
/// Apply a checks callback to the Record.
///
/// Additionally sys validation will be triggered to
/// run again if we weren't holding it.
pub async fn check_and_hold_store_record<F>(
    hash: &ActionHash,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
    incoming_dht_ops_sender: Option<IncomingDhtOpSender>,
    f: F,
) -> SysValidationResult<()>
where
    F: FnOnce(&Record) -> SysValidationResult<()>,
{
    let source = check_and_hold(hash, workspace, network).await?;
    f(source.as_ref())?;
    if let (Some(incoming_dht_ops_sender), Source::Network(record)) =
        (incoming_dht_ops_sender, source)
    {
        incoming_dht_ops_sender.send_store_record(record).await?;
    }
    Ok(())
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

impl IncomingDhtOpSender {
    /// Sends the op to the incoming workflow
    async fn send_op(
        self,
        record: Record,
        make_op: fn(Record) -> Option<(DhtOpHash, DhtOp)>,
    ) -> SysValidationResult<()> {
        if let Some(op) = make_op(record) {
            let ops = vec![op];
            incoming_dht_ops_workflow(self.space.as_ref(), self.sys_validation_trigger, ops, false)
                .await
                .map_err(Box::new)?;
        }
        Ok(())
    }
    async fn send_store_record(self, record: Record) -> SysValidationResult<()> {
        self.send_op(record, make_store_record).await
    }
    async fn send_store_entry(self, record: Record) -> SysValidationResult<()> {
        let is_public_entry = record.action().entry_type().map_or(false, |et| {
            matches!(et.visibility(), EntryVisibility::Public)
        });
        if is_public_entry {
            self.send_op(record, make_store_entry).await?;
        }
        Ok(())
    }
    async fn send_register_add_link(self, record: Record) -> SysValidationResult<()> {
        self.send_op(record, make_register_add_link).await
    }
    async fn send_register_agent_activity(self, record: Record) -> SysValidationResult<()> {
        self.send_op(record, make_register_agent_activity).await
    }
}

/// Where the record was found.
enum Source {
    /// Locally because we are holding it or
    /// because we will be soon
    Local(Record),
    /// On the network.
    /// This means we aren't holding it so
    /// we should add it to our incoming ops
    Network(Record),
}

impl AsRef<Record> for Source {
    fn as_ref(&self) -> &Record {
        match self {
            Source::Local(el) | Source::Network(el) => el,
        }
    }
}

/// Check if we are holding a dependency and
/// run a check callback on the it.
/// This function also returns where the dependency
/// was found so you can decide whether or not to add
/// it to the incoming ops.
async fn check_and_hold<I: Into<AnyDhtHash> + Clone>(
    hash: &I,
    workspace: &SysValidationWorkspace,
    network: HolochainP2pDna,
) -> SysValidationResult<Source> {
    let hash: AnyDhtHash = hash.clone().into();
    // Create a workspace with just the local stores
    let mut local_cascade = workspace.local_cascade();
    if let Some(el) = local_cascade
        .retrieve(hash.clone(), Default::default())
        .await?
    {
        return Ok(Source::Local(el));
    }
    // Create a workspace with just the network
    let mut network_only_cascade = workspace.full_cascade(network);
    match network_only_cascade
        .retrieve(hash.clone(), Default::default())
        .await?
    {
        Some(el) => Ok(Source::Network(el.privatized())),
        None => Err(ValidationOutcome::NotHoldingDep(hash).into()),
    }
}

/// Make a StoreRecord DhtOp from a Record.
/// Note that this can fail if the op is missing an
/// Entry when it was supposed to have one.
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_store_record(record: Record) -> Option<(DhtOpHash, DhtOp)> {
    // Extract the data
    let (shh, record_entry) = record.privatized().into_inner();
    let (action, signature) = shh.into_inner();
    let action = action.into_content();

    // Check the entry
    let maybe_entry_box = record_entry.into_option().map(Box::new);

    // Create the hash and op
    let op = DhtOp::StoreRecord(signature, action, maybe_entry_box);
    let hash = op.to_hash();
    Some((hash, op))
}

/// Make a StoreEntry DhtOp from a Record.
/// Note that this can fail if the op is missing an Entry or
/// the action is the wrong type.
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_store_entry(record: Record) -> Option<(DhtOpHash, DhtOp)> {
    // Extract the data
    let (shh, record_entry) = record.into_inner();
    let (action, signature) = shh.into_inner();

    // Check the entry and exit early if it's not there
    let entry_box = record_entry.into_option()?.into();
    // If the action is the wrong type exit early
    let action = action.into_content().try_into().ok()?;

    // Create the hash and op
    let op = DhtOp::StoreEntry(signature, action, entry_box);
    let hash = op.to_hash();
    Some((hash, op))
}

/// Make a RegisterAddLink DhtOp from a Record.
/// Note that this can fail if the action is the wrong type
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_register_add_link(record: Record) -> Option<(DhtOpHash, DhtOp)> {
    // Extract the data
    let (shh, _) = record.into_inner();
    let (action, signature) = shh.into_inner();

    // If the action is the wrong type exit early
    let action = action.into_content().try_into().ok()?;

    // Create the hash and op
    let op = DhtOp::RegisterAddLink(signature, action);
    let hash = op.to_hash();
    Some((hash, op))
}

/// Make a RegisterAgentActivity DhtOp from a Record.
/// Note that this can fail if the action is the wrong type
///
/// Because adding ops to incoming limbo while we are checking them
/// is only faster then waiting for them through gossip we don't care enough
/// to return an error.
fn make_register_agent_activity(record: Record) -> Option<(DhtOpHash, DhtOp)> {
    // Extract the data
    let (shh, _) = record.into_inner();
    let (action, signature) = shh.into_inner();

    // If the action is the wrong type exit early
    let action = action.into_content();

    // Create the hash and op
    let op = DhtOp::RegisterAgentActivity(signature, action);
    let hash = op.to_hash();
    Some((hash, op))
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
    use holochain_state::test_utils::test_keystore;
    use holochain_zome_types::countersigning::PreflightResponse;
    use matches::assert_matches;

    #[tokio::test(flavor = "multi_thread")]
    pub async fn test_check_countersigning_preflight_response_signature() {
        let keystore = test_keystore();
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

        assert_eq!(
            check_countersigning_preflight_response_signature(&preflight_response)
                .await
                .unwrap(),
            (),
        );
    }
}
