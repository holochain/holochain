//! Defines a Record, the basic unit of Holochain data.

use crate::action::WireActionStatus;
use crate::action::WireDelete;
use crate::action::WireNewEntryAction;
use crate::action::WireUpdateRelationship;
use crate::prelude::*;
use error::RecordGroupError;
use error::RecordGroupResult;
use holochain_keystore::KeystoreError;
use holochain_keystore::LairResult;
use holochain_keystore::MetaLairClient;
use holochain_serialized_bytes::prelude::*;
use holochain_zome_types::entry::EntryHashed;
use std::borrow::Cow;
use std::collections::BTreeSet;

#[allow(missing_docs)]
pub mod error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes, Default)]
/// A condensed version of get record request.
/// This saves bandwidth by removing duplicated and implied data.
pub struct WireRecordOps {
    /// The action this request was for.
    pub action: Option<Judged<SignedAction>>,
    /// Any deletes on the action.
    pub deletes: Vec<Judged<WireDelete>>,
    /// Any updates on the action.
    pub updates: Vec<Judged<WireUpdateRelationship>>,
    /// The entry if there is one.
    pub entry: Option<Entry>,
}

impl WireRecordOps {
    /// Create an empty set of wire record ops.
    pub fn new() -> Self {
        Self::default()
    }
    /// Render these ops to their full types.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        let Self {
            action,
            deletes,
            updates,
            entry,
        } = self;
        let mut ops = Vec::with_capacity(1 + deletes.len() + updates.len());
        if let Some(action) = action {
            let status = action.validation_status();
            let SignedAction(action, signature) = action.data;
            // TODO: If they only need the metadata because they already have
            // the content we could just send the entry hash instead of the
            // SignedAction.
            let entry_hash = action.entry_hash().cloned();
            ops.push(RenderedOp::new(
                action,
                signature,
                status,
                DhtOpType::StoreRecord,
            )?);
            if let Some(entry_hash) = entry_hash {
                for op in deletes {
                    let status = op.validation_status();
                    let op = op.data;
                    let signature = op.signature;
                    let action = Action::Delete(op.delete);

                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        DhtOpType::RegisterDeletedBy,
                    )?);
                }
                for op in updates {
                    let status = op.validation_status();
                    let SignedAction(action, signature) =
                        op.data.into_signed_action(entry_hash.clone());

                    ops.push(RenderedOp::new(
                        action,
                        signature,
                        status,
                        DhtOpType::RegisterUpdatedRecord,
                    )?);
                }
            }
        }
        Ok(RenderedOps {
            entry: entry.map(EntryHashed::from_content_sync),
            ops,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
/// Record without the hashes for sending across the network
/// TODO: Remove this as it's no longer needed.
pub struct WireRecord {
    /// The signed action for this record
    signed_action: SignedAction,
    /// If there is an entry associated with this action it will be here
    maybe_entry: Option<Entry>,
    /// The validation status of this record.
    validation_status: ValidationStatus,
    /// All deletes on this action
    deletes: Vec<WireActionStatus<WireDelete>>,
    /// Any updates on this entry.
    updates: Vec<WireActionStatus<WireUpdateRelationship>>,
}

/// A group of records with a common entry
#[derive(Debug, Clone)]
pub struct RecordGroup<'a> {
    actions: Vec<Cow<'a, SignedActionHashed>>,
    rejected: Vec<Cow<'a, SignedActionHashed>>,
    entry: Cow<'a, EntryHashed>,
}

/// Record with it's status
#[derive(Debug, Clone, derive_more::Constructor)]
pub struct RecordStatus {
    /// The record this status applies to.
    pub record: Record,
    /// Validation status of this record.
    pub status: ValidationStatus,
}

impl<'a> RecordGroup<'a> {
    /// Get the actions and action hashes
    pub fn actions_and_hashes(&self) -> impl Iterator<Item = (&ActionHash, &Action)> {
        self.actions
            .iter()
            .map(|shh| shh.action_address())
            .zip(self.actions.iter().map(|shh| shh.action()))
    }
    /// true if len is zero
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Amount of actions
    pub fn len(&self) -> usize {
        self.actions.len()
    }
    /// The entry's visibility
    pub fn visibility(&self) -> RecordGroupResult<&EntryVisibility> {
        self.actions
            .first()
            .ok_or(RecordGroupError::Empty)?
            .action()
            .entry_data()
            .map(|(_, et)| et.visibility())
            .ok_or(RecordGroupError::MissingEntryData)
    }
    /// The entry hash
    pub fn entry_hash(&self) -> &EntryHash {
        self.entry.as_hash()
    }
    /// The entry with hash
    pub fn entry_hashed(&self) -> EntryHashed {
        self.entry.clone().into_owned()
    }
    /// Get owned iterator of signed actions
    pub fn owned_signed_actions(&self) -> impl Iterator<Item = SignedActionHashed> + 'a {
        self.actions
            .clone()
            .into_iter()
            .chain(self.rejected.clone().into_iter())
            .map(|shh| shh.into_owned())
    }

    /// Get the valid action hashes
    pub fn valid_hashes(&self) -> impl Iterator<Item = &ActionHash> {
        self.actions.iter().map(|shh| shh.action_address())
    }

    /// Get the rejected action hashes
    pub fn rejected_hashes(&self) -> impl Iterator<Item = &ActionHash> {
        self.rejected.iter().map(|shh| shh.action_address())
    }

    /// Create an record group from wire actions and an entry
    pub fn from_wire_records<I: IntoIterator<Item = WireActionStatus<WireNewEntryAction>>>(
        actions_iter: I,
        entry_type: EntryType,
        entry: Entry,
    ) -> RecordGroupResult<RecordGroup<'a>> {
        let iter = actions_iter.into_iter();
        let mut valid = Vec::with_capacity(iter.size_hint().0);
        let mut rejected = Vec::with_capacity(iter.size_hint().0);
        let entry = entry.into_hashed();
        let entry_hash = entry.as_hash().clone();
        let entry = Cow::Owned(entry);
        for wire in iter {
            match wire.validation_status {
                ValidationStatus::Valid => valid.push(Cow::Owned(
                    wire.action
                        .into_action(entry_type.clone(), entry_hash.clone()),
                )),
                ValidationStatus::Rejected => rejected.push(Cow::Owned(
                    wire.action
                        .into_action(entry_type.clone(), entry_hash.clone()),
                )),
                ValidationStatus::Abandoned => todo!(),
            }
        }

        Ok(Self {
            actions: valid,
            rejected,
            entry,
        })
    }
}

/// Responses from a dht get.
/// These vary is size depending on the level of metadata required
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum GetRecordResponse {
    /// Can be combined with any other metadata monotonically
    GetEntryFull(Option<Box<RawGetEntryResponse>>),
    /// Placeholder for more optimized get
    GetEntryPartial,
    /// Placeholder for more optimized get
    GetEntryCollapsed,
    /// Get a single record
    /// Can be combined with other metadata monotonically
    GetAction(Option<Box<WireRecord>>),
}

/// This type gives full metadata that can be combined
/// monotonically with other metadata and the actual data
// in the most compact way that also avoids multiple calls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct RawGetEntryResponse {
    /// The live actions from this authority.
    /// These can be collapsed to NewEntryActionLight
    /// Which omits the EntryHash and EntryType,
    /// saving 32 bytes each
    pub live_actions: BTreeSet<WireActionStatus<WireNewEntryAction>>,
    /// just the hashes of actions to delete
    // TODO: Perf could just send the ActionHash of the
    // action being deleted but we would need to only ever store
    // if there was an action delete in our MetadataBuf and
    // not the delete action hash as we do now.
    pub deletes: Vec<WireActionStatus<WireDelete>>,
    /// Any updates on this entry.
    /// Note you will need to ask for "all_live_actions_with_metadata"
    /// to get this back
    pub updates: Vec<WireActionStatus<WireUpdateRelationship>>,
    /// The entry shared across all actions
    pub entry: Entry,
    /// The entry_type shared across all actions
    pub entry_type: EntryType,
}

impl RawGetEntryResponse {
    /// Creates the response from a set of chain records
    /// that share the same entry with any deletes.
    /// Note: It's the callers responsibility to check that
    /// records all have the same entry. This is not checked
    /// due to the performance cost.
    /// ### Panics
    /// If the records are not an action of Create or EntryDelete
    /// or there is no entry or the entry hash is different
    pub fn from_records<E>(
        records: E,
        deletes: Vec<WireActionStatus<WireDelete>>,
        updates: Vec<WireActionStatus<WireUpdateRelationship>>,
    ) -> Option<Self>
    where
        E: IntoIterator<Item = RecordStatus>,
    {
        let mut records = records.into_iter();
        records.next().map(|RecordStatus { record, status }| {
            let mut live_actions = BTreeSet::new();
            let (new_entry_action, entry_type, entry) = Self::from_record(record);
            live_actions.insert(WireActionStatus::new(new_entry_action, status));
            let r = Self {
                live_actions,
                deletes,
                updates,
                entry,
                entry_type,
            };
            records.fold(r, |mut response, RecordStatus { record, status }| {
                let (new_entry_action, entry_type, entry) = Self::from_record(record);
                debug_assert_eq!(response.entry, entry);
                debug_assert_eq!(response.entry_type, entry_type);
                response
                    .live_actions
                    .insert(WireActionStatus::new(new_entry_action, status));
                response
            })
        })
    }

    fn from_record(record: Record) -> (WireNewEntryAction, EntryType, Entry) {
        let (shh, entry) = record.into_inner();
        let entry = entry
            .into_option()
            .expect("Get entry responses cannot be created without entries");
        let (action, signature) = shh.into_inner();
        let (new_entry_action, entry_type) = match action.into_content() {
            Action::Create(ec) => {
                let et = ec.entry_type.clone();
                (WireNewEntryAction::Create((ec, signature).into()), et)
            }
            Action::Update(eu) => {
                let et = eu.entry_type.clone();
                (WireNewEntryAction::Update((eu, signature).into()), et)
            }
            h => panic!(
                "Get entry responses cannot be created from actions
                    other then Create or Update.
                    Tried to with: {:?}",
                h
            ),
        };
        (new_entry_action, entry_type, entry)
    }
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait RecordExt {
    /// Validate the signature matches the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[async_trait::async_trait]
impl RecordExt for Record {
    /// Validates a chain record
    async fn validate(&self) -> Result<(), KeystoreError> {
        self.signed_action().validate().await?;

        //TODO: make sure that any cases around entry existence are valid:
        //      SourceChainError::InvalidStructure(ActionAndEntryMismatch(address)),
        Ok(())
    }
}

/// Extension trait to keep zome types minimal
#[async_trait::async_trait]
pub trait SignedActionHashedExt {
    /// Create a hash from data
    fn from_content_sync(signed_action: SignedAction) -> SignedActionHashed;
    /// Sign some content
    #[allow(clippy::new_ret_no_self)]
    async fn sign(
        keystore: &MetaLairClient,
        action: ActionHashed,
    ) -> LairResult<SignedActionHashed>;
    /// Validate the data
    async fn validate(&self) -> Result<(), KeystoreError>;
}

#[allow(missing_docs)]
#[async_trait::async_trait]
impl SignedActionHashedExt for SignedActionHashed {
    fn from_content_sync(signed_action: SignedAction) -> Self
    where
        Self: Sized,
    {
        let (action, signature) = signed_action.into();
        Self::with_presigned(action.into_hashed(), signature)
    }
    /// SignedAction constructor
    async fn sign(keystore: &MetaLairClient, action: ActionHashed) -> LairResult<Self> {
        let signature = action.author().sign(keystore, &*action).await?;
        Ok(Self::with_presigned(action, signature))
    }

    /// Validates a signed action
    async fn validate(&self) -> Result<(), KeystoreError> {
        if !self
            .action()
            .author()
            .verify_signature(self.signature(), self.action())
            .await
        {
            return Err(KeystoreError::InvalidSignature(
                self.signature().clone(),
                format!("action {:?}", self.action_address()),
            ));
        }
        Ok(())
    }
}

impl WireRecord {
    /// Convert into a [Record], deletes and updates when receiving from the network
    pub fn into_parts(self) -> (RecordStatus, Vec<RecordStatus>, Vec<RecordStatus>) {
        let entry_hash = self.signed_action.action().entry_hash().cloned();
        let action = Record::new(
            SignedActionHashed::from_content_sync(self.signed_action),
            self.maybe_entry,
        );
        let deletes = self
            .deletes
            .into_iter()
            .map(WireActionStatus::<WireDelete>::into_record_status)
            .collect();
        let updates = self
            .updates
            .into_iter()
            .map(|u| {
                let entry_hash = entry_hash
                    .clone()
                    .expect("Updates cannot be on actions that do not have entries");
                u.into_record_status(entry_hash)
            })
            .collect();
        (
            RecordStatus::new(action, self.validation_status),
            deletes,
            updates,
        )
    }
    /// Convert from a [Record] when sending to the network
    pub fn from_record(
        e: RecordStatus,
        deletes: Vec<WireActionStatus<WireDelete>>,
        updates: Vec<WireActionStatus<WireUpdateRelationship>>,
    ) -> Self {
        let RecordStatus { record, status } = e;
        let (signed_action, maybe_entry) = record.into_inner();
        Self {
            signed_action: signed_action.into(),
            // TODO: consider refactoring WireRecord to use RecordEntry
            // instead of Option<Entry>
            maybe_entry: maybe_entry.into_option(),
            validation_status: status,
            deletes,
            updates,
        }
    }

    /// Get the entry hash if there is one
    pub fn entry_hash(&self) -> Option<&EntryHash> {
        self.signed_action
            .action()
            .entry_data()
            .map(|(hash, _)| hash)
    }
}

#[cfg(test)]
mod tests {
    use super::SignedAction;
    use super::SignedActionHashed;
    use crate::prelude::*;
    use ::fixt::prelude::*;
    use holo_hash::HasHash;
    use holo_hash::HoloHashed;

    #[tokio::test(flavor = "multi_thread")]
    async fn test_signed_action_roundtrip() {
        let signature = SignatureFixturator::new(Unpredictable).next().unwrap();
        let action = ActionFixturator::new(Unpredictable).next().unwrap();
        let signed_action = SignedAction(action, signature);
        let hashed: HoloHashed<SignedAction> = HoloHashed::from_content_sync(signed_action);
        let HoloHashed {
            content: SignedAction(action, signature),
            hash,
        } = hashed.clone();
        let shh = SignedActionHashed {
            hashed: ActionHashed::with_pre_hashed(action, hash),
            signature,
        };

        assert_eq!(shh.action_address(), hashed.as_hash());

        let round = HoloHashed {
            content: SignedAction(shh.action().clone(), shh.signature().clone()),
            hash: shh.action_address().clone(),
        };

        assert_eq!(hashed, round);
    }
}
