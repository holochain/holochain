//! Holochain's [`Action`] and its variations.
//!
//! All action variations contain the fields `author` and `timestamp`.
//! Furthermore, all variations besides pub struct `Dna` (which is the first action
//! in a chain) contain the field `prev_action`.

#![allow(missing_docs)]

use crate::prelude::*;
use conversions::WrongActionError;
use derive_more::From;
use holo_hash::EntryHash;
use holochain_zome_types::op::EntryCreationAction;

// The legacy per-variant `Action` enum. `crate::prelude::Action` resolves to
// the v2 `ActionHeader` + `ActionData` shape, so this explicit import shadows
// it for the legacy `NewEntryAction`/`NewEntryActionRef` machinery below,
// which is not yet migrated to v2 (still consumed by sys-validation,
// integration, the source chain, and the cascade).
use holochain_zome_types::dependencies::holochain_integrity_types::action::Action;

/// A action of one of the two types that create a new entry.
#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, SerializedBytes, Hash, derive_more::From,
)]
pub enum NewEntryAction {
    /// A action which simply creates a new entry
    Create(Create),
    /// A action which creates a new entry that is semantically related to a
    /// previously created entry or action
    Update(Update),
}

/// Same as NewEntryAction but takes actions as reference
#[allow(missing_docs)]
#[derive(Debug, From)]
pub enum NewEntryActionRef<'a> {
    Create(&'a Create),
    Update(&'a Update),
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
    pub fn timestamp(&self) -> holochain_zome_types::timestamp::Timestamp {
        match self {
            NewEntryAction::Create(Create { timestamp, .. })
            | NewEntryAction::Update(Update { timestamp, .. }) => *timestamp,
        }
    }

    /// Get the author of this action
    pub fn author(&self) -> &AgentPubKey {
        match self {
            NewEntryAction::Create(Create { author, .. })
            | NewEntryAction::Update(Update { author, .. }) => author,
        }
    }

    /// Get the action_seq of this action
    pub fn action_seq(&self) -> u32 {
        match self {
            NewEntryAction::Create(Create { action_seq, .. })
            | NewEntryAction::Update(Update { action_seq, .. }) => *action_seq,
        }
    }
}

impl From<NewEntryAction> for Action {
    fn from(a: NewEntryAction) -> Self {
        match a {
            NewEntryAction::Create(a) => Action::Create(a),
            NewEntryAction::Update(a) => Action::Update(a),
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

impl TryFrom<Action> for NewEntryAction {
    type Error = WrongActionError;
    fn try_from(value: Action) -> Result<Self, Self::Error> {
        match value {
            Action::Create(a) => Ok(NewEntryAction::Create(a)),
            Action::Update(a) => Ok(NewEntryAction::Update(a)),
            _ => Err(WrongActionError(format!("{value:?}"))),
        }
    }
}

impl<'a> TryFrom<&'a Action> for NewEntryActionRef<'a> {
    type Error = WrongActionError;
    fn try_from(value: &'a Action) -> Result<Self, Self::Error> {
        match value {
            Action::Create(a) => Ok(NewEntryActionRef::Create(a)),
            Action::Update(a) => Ok(NewEntryActionRef::Update(a)),
            _ => Err(WrongActionError(format!("{value:?}"))),
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
    fn test_action_json_roundtrip() {
        let orig: Action = Dna::from_builder(
            fake_dna_hash(1),
            ActionBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
        )
        .into();
        let orig = ActionHashed::from_content_sync(orig);
        let json = serde_json::to_string(&orig).unwrap();
        dbg!(&json);
        let res: ActionHashed = serde_json::from_str(&json).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_create_entry_msgpack_roundtrip() {
        let orig: Action = Create::from_builder(
            ActionBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_hash(1),
        )
        .into();
        let bytes = holochain_serialized_bytes::encode(&orig).unwrap();
        println!("{bytes:?}");
        let res: Action = holochain_serialized_bytes::decode(&bytes).unwrap();
        assert_eq!(orig, res);
    }

    #[test]
    fn test_create_entry_serializedbytes_roundtrip() {
        let orig: Action = Create::from_builder(
            ActionBuilderCommonFixturator::new(Unpredictable)
                .next()
                .unwrap(),
            EntryType::App(AppEntryDef::new(
                0.into(),
                0.into(),
                EntryVisibility::Public,
            )),
            fake_entry_hash(1),
        )
        .into();
        let bytes: SerializedBytes = orig.clone().try_into().unwrap();
        let res: Action = bytes.try_into().unwrap();
        assert_eq!(orig, res);
    }
}
