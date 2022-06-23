//! Holochain's [`Action`] and its variations.
//!
//! All action variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first action
//! in a chain) contain the field `prev_action`.

#![allow(missing_docs)]

use crate::prelude::*;
use crate::record::RecordStatus;
use crate::record::SignedActionHashedExt;
use conversions::WrongActionError;
use derive_more::From;
use holo_hash::EntryHash;
use holochain_zome_types::op::EntryCreationAction;
use holochain_zome_types::prelude::*;

#[cfg(feature = "contrafact")]
pub mod facts;

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash, derive_more::From,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
/// A action of one of the two types that create a new entry.
pub enum NewEntryAction {
    /// A action which simply creates a new entry
    Create(Create),
    /// A action which creates a new entry that is semantically related to a
    /// previously created entry or action
    Update(Update),
}

#[allow(missing_docs)]
#[derive(Debug, From)]
/// Same as NewEntryAction but takes actions as reference
pub enum NewEntryActionRef<'a> {
    Create(&'a Create),
    Update(&'a Update),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
/// A action of one of the two types that create a new entry.
pub enum WireNewEntryAction {
    Create(WireCreate),
    Update(WireUpdate),
}

#[derive(
    Debug, Clone, derive_more::Constructor, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd,
)]
/// A action of one of the two types that create a new entry.
pub struct WireActionStatus<W> {
    /// Skinny action for sending over the wire.
    pub action: W,
    /// Validation status of this action.
    pub validation_status: ValidationStatus,
}

/// The minimum unique data for Create actions
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireCreate {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub action_seq: u32,
    pub prev_action: ActionHash,
    pub signature: Signature,
    pub weight: EntryRateWeight,
}

/// The minimum unique data for Update actions
/// that share a common entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Ord, PartialOrd)]
pub struct WireUpdate {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub action_seq: u32,
    pub prev_action: ActionHash,
    pub original_entry_address: EntryHash,
    pub original_action_address: ActionHash,
    pub signature: Signature,
    pub weight: EntryRateWeight,
}

/// This type is used when sending updates from the
/// original entry authority to someone asking for
/// metadata on that original entry.
/// ## How updates work
/// `Update` actions create both a new entry and
/// a metadata relationship on the original entry.
/// This wire data represents the metadata relationship
/// which is stored on the original entry, i.e. this represents
/// the "forward" reference from the original entry to the new entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct WireUpdateRelationship {
    /// Timestamp is first so that deriving Ord results in
    /// order by time
    pub timestamp: holochain_zome_types::timestamp::Timestamp,
    pub author: AgentPubKey,
    pub action_seq: u32,
    pub prev_action: ActionHash,
    /// Address of the original entry action
    pub original_action_address: ActionHash,
    /// The entry that this update created
    pub new_entry_address: EntryHash,
    /// The entry type of the entry that this action created
    pub new_entry_type: EntryType,
    pub signature: Signature,
    pub weight: EntryRateWeight,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes)]
pub struct WireDelete {
    pub delete: Delete,
    pub signature: Signature,
}

impl NewEntryAction {
    /// Get the entry on this action
    pub fn entry(&self) -> &EntryHash {
        match self {
            NewEntryAction::Create(Create { entry_hash, .. })
            | NewEntryAction::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }

    /// Get the entry type on this action
    pub fn entry_type(&self) -> &EntryType {
        match self {
            NewEntryAction::Create(Create { entry_type, .. })
            | NewEntryAction::Update(Update { entry_type, .. }) => entry_type,
        }
    }

    /// Get the visibility of this action
    pub fn visibility(&self) -> &EntryVisibility {
        match self {
            NewEntryAction::Create(Create { entry_type, .. })
            | NewEntryAction::Update(Update { entry_type, .. }) => entry_type.visibility(),
        }
    }

    /// Get the timestamp of this action
    pub fn timestamp(&self) -> &holochain_zome_types::timestamp::Timestamp {
        match self {
            NewEntryAction::Create(Create { timestamp, .. })
            | NewEntryAction::Update(Update { timestamp, .. }) => timestamp,
        }
    }
}

impl From<NewEntryAction> for Action {
    fn from(h: NewEntryAction) -> Self {
        match h {
            NewEntryAction::Create(h) => Action::Create(h),
            NewEntryAction::Update(h) => Action::Update(h),
        }
    }
}

impl From<NewEntryAction> for EntryCreationAction {
    fn from(action: NewEntryAction) -> Self {
        match action {
            NewEntryAction::Create(create) => EntryCreationAction::Create(create),
            NewEntryAction::Update(update) => EntryCreationAction::Update(update),
        }
    }
}

impl From<(Create, Signature)> for WireCreate {
    fn from((ec, signature): (Create, Signature)) -> Self {
        Self {
            timestamp: ec.timestamp,
            author: ec.author,
            action_seq: ec.action_seq,
            prev_action: ec.prev_action,
            signature,
            weight: ec.weight,
        }
    }
}

impl From<(Update, Signature)> for WireUpdate {
    fn from((eu, signature): (Update, Signature)) -> Self {
        Self {
            timestamp: eu.timestamp,
            author: eu.author,
            action_seq: eu.action_seq,
            prev_action: eu.prev_action,
            original_entry_address: eu.original_entry_address,
            original_action_address: eu.original_action_address,
            signature,
            weight: eu.weight,
        }
    }
}

impl WireDelete {
    pub fn into_record(self) -> Record {
        Record::new(
            SignedActionHashed::from_content_sync(SignedAction(self.delete.into(), self.signature)),
            None,
        )
    }
}

impl WireUpdateRelationship {
    /// Recreate the Update Record without an Entry.
    /// Useful for creating dht ops
    pub fn into_record(self, original_entry_address: EntryHash) -> Record {
        Record::new(
            SignedActionHashed::from_content_sync(self.into_signed_action(original_entry_address)),
            None,
        )
    }

    /// Render the [`SignedAction`] from the wire type
    pub fn into_signed_action(self, original_entry_address: EntryHash) -> SignedAction {
        let eu = Update {
            author: self.author,
            timestamp: self.timestamp,
            action_seq: self.action_seq,
            prev_action: self.prev_action,
            original_action_address: self.original_action_address,
            original_entry_address,
            entry_type: self.new_entry_type,
            entry_hash: self.new_entry_address,
            weight: self.weight,
        };
        SignedAction(Action::Update(eu), self.signature)
    }
}

impl NewEntryActionRef<'_> {
    pub fn entry_type(&self) -> &EntryType {
        match self {
            NewEntryActionRef::Create(Create { entry_type, .. })
            | NewEntryActionRef::Update(Update { entry_type, .. }) => entry_type,
        }
    }
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            NewEntryActionRef::Create(Create { entry_hash, .. })
            | NewEntryActionRef::Update(Update { entry_hash, .. }) => entry_hash,
        }
    }
    pub fn to_new_entry_action(&self) -> NewEntryAction {
        match self {
            NewEntryActionRef::Create(create) => NewEntryAction::Create((*create).to_owned()),
            NewEntryActionRef::Update(update) => NewEntryAction::Update((*update).to_owned()),
        }
    }
}

impl TryFrom<SignedActionHashed> for WireDelete {
    type Error = WrongActionError;
    fn try_from(shh: SignedActionHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_inner();
        Ok(Self {
            delete: h.into_content().try_into()?,
            signature,
        })
    }
}

impl TryFrom<SignedAction> for WireDelete {
    type Error = WrongActionError;
    fn try_from(sh: SignedAction) -> Result<Self, Self::Error> {
        let SignedAction(h, signature) = sh;
        Ok(Self {
            delete: h.try_into()?,
            signature,
        })
    }
}

impl TryFrom<SignedActionHashed> for WireUpdate {
    type Error = WrongActionError;
    fn try_from(shh: SignedActionHashed) -> Result<Self, Self::Error> {
        let (h, signature) = shh.into_inner();
        let d: Update = h.into_content().try_into()?;
        Ok(Self {
            signature,
            timestamp: d.timestamp,
            author: d.author,
            action_seq: d.action_seq,
            prev_action: d.prev_action,
            original_entry_address: d.original_entry_address,
            original_action_address: d.original_action_address,
            weight: d.weight,
        })
    }
}

impl TryFrom<SignedActionHashed> for WireUpdateRelationship {
    type Error = WrongActionError;
    fn try_from(shh: SignedActionHashed) -> Result<Self, Self::Error> {
        let (h, s) = shh.into_inner();
        SignedAction(h.into_content(), s).try_into()
    }
}

impl TryFrom<SignedAction> for WireUpdateRelationship {
    type Error = WrongActionError;
    fn try_from(sh: SignedAction) -> Result<Self, Self::Error> {
        let SignedAction(h, signature) = sh;
        let d: Update = h.try_into()?;
        Ok(Self {
            signature,
            timestamp: d.timestamp,
            author: d.author,
            action_seq: d.action_seq,
            prev_action: d.prev_action,
            original_action_address: d.original_action_address,
            new_entry_address: d.entry_hash,
            new_entry_type: d.entry_type,
            weight: d.weight,
        })
    }
}

impl WireNewEntryAction {
    pub fn into_record(self, entry_type: EntryType, entry: Entry) -> Record {
        let entry_hash = EntryHash::with_data_sync(&entry);
        Record::new(self.into_action(entry_type, entry_hash), Some(entry))
    }

    pub fn into_action(self, entry_type: EntryType, entry_hash: EntryHash) -> SignedActionHashed {
        SignedActionHashed::from_content_sync(self.into_signed_action(entry_type, entry_hash))
    }

    pub fn into_signed_action(self, entry_type: EntryType, entry_hash: EntryHash) -> SignedAction {
        match self {
            WireNewEntryAction::Create(ec) => {
                let signature = ec.signature;
                let ec = Create {
                    author: ec.author,
                    timestamp: ec.timestamp,
                    action_seq: ec.action_seq,
                    prev_action: ec.prev_action,
                    weight: ec.weight,
                    entry_type,
                    entry_hash,
                };
                SignedAction(ec.into(), signature)
            }
            WireNewEntryAction::Update(eu) => {
                let signature = eu.signature;
                let eu = Update {
                    author: eu.author,
                    timestamp: eu.timestamp,
                    action_seq: eu.action_seq,
                    prev_action: eu.prev_action,
                    original_entry_address: eu.original_entry_address,
                    original_action_address: eu.original_action_address,
                    weight: eu.weight,
                    entry_type,
                    entry_hash,
                };
                SignedAction(eu.into(), signature)
            }
        }
    }
}

impl WireActionStatus<WireNewEntryAction> {
    pub fn into_record_status(self, entry_type: EntryType, entry: Entry) -> RecordStatus {
        RecordStatus::new(
            self.action.into_record(entry_type, entry),
            self.validation_status,
        )
    }
}

impl WireActionStatus<WireUpdateRelationship> {
    pub fn into_record_status(self, entry_hash: EntryHash) -> RecordStatus {
        RecordStatus::new(self.action.into_record(entry_hash), self.validation_status)
    }
}

impl WireActionStatus<WireDelete> {
    pub fn into_record_status(self) -> RecordStatus {
        RecordStatus::new(self.action.into_record(), self.validation_status)
    }
}

impl<H, W, E> TryFrom<(H, ValidationStatus)> for WireActionStatus<W>
where
    E: Into<ActionError>,
    H: TryInto<W, Error = E>,
{
    type Error = ActionError;

    fn try_from(value: (H, ValidationStatus)) -> Result<Self, Self::Error> {
        Ok(Self::new(value.0.try_into().map_err(Into::into)?, value.1))
    }
}

impl TryFrom<SignedActionHashed> for WireNewEntryAction {
    type Error = ActionError;
    fn try_from(shh: SignedActionHashed) -> Result<Self, Self::Error> {
        let action = shh.hashed.content;
        let signature = shh.signature;
        match action {
            Action::Create(ec) => Ok(Self::Create((ec, signature).into())),
            Action::Update(eu) => Ok(Self::Update((eu, signature).into())),
            _ => Err(ActionError::NotNewEntry),
        }
    }
}

impl TryFrom<SignedAction> for WireNewEntryAction {
    type Error = ActionError;
    fn try_from(sh: SignedAction) -> Result<Self, Self::Error> {
        let (action, s) = sh.into();
        match action {
            Action::Create(ec) => Ok(Self::Create((ec, s).into())),
            Action::Update(eu) => Ok(Self::Update((eu, s).into())),
            _ => Err(ActionError::NotNewEntry),
        }
    }
}

impl TryFrom<Action> for NewEntryAction {
    type Error = WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::Create(h) => Ok(NewEntryAction::Create(h)),
            Action::Update(h) => Ok(NewEntryAction::Update(h)),
            _ => Err(WrongActionError(format!("{:?}", value))),
        }
    }
}

impl<'a> TryFrom<&'a Action> for NewEntryActionRef<'a> {
    type Error = WrongActionError;
    fn try_from(value: &'a Action) -> Result<Self, Self::Error> {
        match value {
            Action::Create(h) => Ok(NewEntryActionRef::Create(h)),
            Action::Update(h) => Ok(NewEntryActionRef::Update(h)),
            _ => Err(WrongActionError(format!("{:?}", value))),
        }
    }
}

impl<'a> From<&'a NewEntryAction> for NewEntryActionRef<'a> {
    fn from(n: &'a NewEntryAction) -> Self {
        match n {
            NewEntryAction::Create(ec) => NewEntryActionRef::Create(ec),
            NewEntryAction::Update(eu) => NewEntryActionRef::Update(eu),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixt::ActionBuilderCommonFixturator;
    use crate::test_utils::fake_dna_hash;
    use crate::test_utils::fake_entry_hash;
    use ::fixt::prelude::Unpredictable;

    #[test]
    fn test_action_msgpack_roundtrip() {
        let orig: Action = Dna::from_builder(
            fake_dna_hash(1),
            ActionBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
        )
        .into();
        let bytes = holochain_serialized_bytes::encode(&orig).unwrap();
        let res: Action = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_create_entry_msgpack_roundtrip() {
        let orig: Action = Create::from_builder(
            ActionBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(0.into(), EntryVisibility::Public)),
            fake_entry_hash(1).into(),
        )
        .into();
        let bytes = holochain_serialized_bytes::encode(&orig).unwrap();
        println!("{:?}", bytes);
        let res: Action = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_create_entry_serializedbytes_roundtrip() {
        let orig: Action = Create::from_builder(
            ActionBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryType::new(0.into(), EntryVisibility::Public)),
            fake_entry_hash(1).into(),
        )
        .into();
        let bytes: SerializedBytes = orig.clone().try_into().unwrap();
        let res: Action = bytes.try_into().unwrap();
        assert_eq!(orig, res);
    }
}
