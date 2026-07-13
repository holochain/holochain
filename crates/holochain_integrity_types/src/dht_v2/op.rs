//! The DHT [`Op`] model, built around `ActionData`.

use super::{Action, ActionData, ActionType, Record};
use crate::action::conversions::WrongActionError;
use crate::record::SignedHashed;
use crate::{Entry, EntryType};
use holo_hash::{ActionHash, AgentPubKey, EntryHash};
use holochain_serialized_bytes::prelude::*;
use holochain_timestamp::Timestamp;

/// A DHT operation produced by an action and validated by an authority.
///
/// Variants carry the [`SignedHashed<Action>`] directly; consumers inspect
/// `action.hashed.content.data` ([`ActionData`]) to discriminate, rather than
/// matching distinct typed per-variant action structs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub enum Op {
    /// Stores a [`Record`] (validated by the action authority).
    StoreRecord(StoreRecord),
    /// Stores an [`Entry`] (validated by the entry authority). The action's
    /// [`ActionData`] is `Create` or `Update`.
    StoreEntry(StoreEntry),
    /// Registers an update against an entry. The action's data is `Update`.
    RegisterUpdate(RegisterUpdate),
    /// Registers a delete against an entry. The action's data is `Delete`.
    RegisterDelete(RegisterDelete),
    /// Registers an action on an agent's source chain (validated by the chain
    /// authority); produced for every action.
    RegisterAgentActivity(RegisterAgentActivity),
    /// Registers a link. The action's data is `CreateLink`.
    RegisterCreateLink(RegisterCreateLink),
    /// Registers a link deletion. The action's data is `DeleteLink`.
    RegisterDeleteLink(RegisterDeleteLink),
}

/// See [`Op::StoreRecord`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, SerializedBytes)]
pub struct StoreRecord {
    /// The record being stored.
    pub record: Record,
}

/// See [`Op::StoreEntry`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct StoreEntry {
    /// The signed action whose data is `Create` or `Update`.
    pub action: SignedHashed<Action>,
    /// The entry being stored.
    pub entry: Entry,
}

/// See [`Op::RegisterUpdate`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct RegisterUpdate {
    /// The signed `Update` action.
    pub update: SignedHashed<Action>,
    /// The new entry, absent when the entry is private.
    pub new_entry: Option<Entry>,
}

/// See [`Op::RegisterDelete`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct RegisterDelete {
    /// The signed `Delete` action.
    pub delete: SignedHashed<Action>,
}

/// See [`Op::RegisterAgentActivity`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct RegisterAgentActivity {
    /// The signed action being registered.
    pub action: SignedHashed<Action>,
    /// Optionally cached entry for agent-activity authorities.
    pub cached_entry: Option<Entry>,
}

/// See [`Op::RegisterCreateLink`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct RegisterCreateLink {
    /// The signed `CreateLink` action.
    pub create_link: SignedHashed<Action>,
}

/// See [`Op::RegisterDeleteLink`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
pub struct RegisterDeleteLink {
    /// The signed `DeleteLink` action.
    pub delete_link: SignedHashed<Action>,
    /// The original `CreateLink` action content being deleted.
    pub create_link: Action,
}

impl StoreEntry {
    /// Construct, validating that the action's data creates an entry.
    pub fn new(action: SignedHashed<Action>, entry: Entry) -> Result<Self, WrongActionError> {
        match &action.hashed.content.data {
            ActionData::Create(_) | ActionData::Update(_) => Ok(Self { action, entry }),
            other => Err(WrongActionError(format!(
                "StoreEntry requires Create or Update action data, got {:?}",
                other.action_type()
            ))),
        }
    }
}

impl RegisterUpdate {
    /// Construct, validating that the action's data is an `Update`.
    pub fn new(
        update: SignedHashed<Action>,
        new_entry: Option<Entry>,
    ) -> Result<Self, WrongActionError> {
        match &update.hashed.content.data {
            ActionData::Update(_) => Ok(Self { update, new_entry }),
            other => Err(WrongActionError(format!(
                "RegisterUpdate requires Update action data, got {:?}",
                other.action_type()
            ))),
        }
    }
}

impl RegisterDelete {
    /// Construct, validating that the action's data is a `Delete`.
    pub fn new(delete: SignedHashed<Action>) -> Result<Self, WrongActionError> {
        match &delete.hashed.content.data {
            ActionData::Delete(_) => Ok(Self { delete }),
            other => Err(WrongActionError(format!(
                "RegisterDelete requires Delete action data, got {:?}",
                other.action_type()
            ))),
        }
    }
}

impl RegisterCreateLink {
    /// Construct, validating that the action's data is a `CreateLink`.
    pub fn new(create_link: SignedHashed<Action>) -> Result<Self, WrongActionError> {
        match &create_link.hashed.content.data {
            ActionData::CreateLink(_) => Ok(Self { create_link }),
            other => Err(WrongActionError(format!(
                "RegisterCreateLink requires CreateLink action data, got {:?}",
                other.action_type()
            ))),
        }
    }
}

impl RegisterDeleteLink {
    /// Construct, validating the delete action is a `DeleteLink`, the referenced
    /// original action is a `CreateLink`, and the two share a base address (so
    /// the delete actually targets the supplied create-link's base).
    pub fn new(
        delete_link: SignedHashed<Action>,
        create_link: Action,
    ) -> Result<Self, WrongActionError> {
        match (&delete_link.hashed.content.data, &create_link.data) {
            (ActionData::DeleteLink(dl), ActionData::CreateLink(cl)) => {
                if dl.base_address != cl.base_address {
                    return Err(WrongActionError(
                        "RegisterDeleteLink requires the DeleteLink and CreateLink to share a base address".into(),
                    ));
                }
                Ok(Self {
                    delete_link,
                    create_link,
                })
            }
            (dl, cl) => Err(WrongActionError(format!(
                "RegisterDeleteLink requires DeleteLink and CreateLink action data, got {:?} and {:?}",
                dl.action_type(),
                cl.action_type()
            ))),
        }
    }
}

impl Op {
    /// The signed action backing this op.
    fn signed_action(&self) -> &SignedHashed<Action> {
        match self {
            Op::StoreRecord(StoreRecord { record }) => &record.signed_action,
            Op::StoreEntry(StoreEntry { action, .. }) => action,
            Op::RegisterUpdate(RegisterUpdate { update, .. }) => update,
            Op::RegisterDelete(RegisterDelete { delete }) => delete,
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => action,
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => create_link,
            Op::RegisterDeleteLink(RegisterDeleteLink { delete_link, .. }) => delete_link,
        }
    }

    /// The author of this op's action.
    pub fn author(&self) -> &AgentPubKey {
        &self.signed_action().hashed.content.header.author
    }

    /// The authored timestamp of this op's action.
    pub fn timestamp(&self) -> Timestamp {
        self.signed_action().hashed.content.header.timestamp
    }

    /// The source-chain sequence of this op's action.
    pub fn action_seq(&self) -> u32 {
        self.signed_action().hashed.content.header.action_seq
    }

    /// The previous action hash, if any.
    pub fn prev_action(&self) -> Option<&ActionHash> {
        self.signed_action()
            .hashed
            .content
            .header
            .prev_action
            .as_ref()
    }

    /// The action type of this op.
    pub fn action_type(&self) -> ActionType {
        self.signed_action().hashed.content.data.action_type()
    }

    /// The action hash of this op.
    pub fn action_hash(&self) -> &ActionHash {
        self.signed_action().as_hash()
    }

    /// The entry hash and type, for ops whose action creates an entry.
    pub fn entry_data(&self) -> Option<(&EntryHash, &EntryType)> {
        match &self.signed_action().hashed.content.data {
            ActionData::Create(d) => Some((&d.entry_hash, &d.entry_type)),
            ActionData::Update(d) => Some((&d.entry_hash, &d.entry_type)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dht_v2::{Action, ActionData, ActionHeader, CreateData, DeleteData, DeleteLinkData};
    use crate::record::SignedHashed;
    use crate::signature::Signature;
    use crate::EntryType;
    use holo_hash::{ActionHash, AgentPubKey, EntryHash, HoloHashed};

    fn signed_action(data: ActionData) -> SignedHashed<Action> {
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_timestamp::Timestamp::from_micros(7),
                action_seq: 1,
                prev_action: Some(ActionHash::from_raw_36(vec![2u8; 36])),
            },
            data,
        };
        let hash = ActionHash::from_raw_36(vec![9u8; 36]);
        SignedHashed::with_presigned(
            HoloHashed::with_pre_hashed(action, hash),
            Signature([0u8; 64]),
        )
    }

    fn create_data() -> ActionData {
        ActionData::Create(CreateData {
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
        })
    }

    fn delete_data() -> ActionData {
        ActionData::Delete(DeleteData {
            deletes_address: ActionHash::from_raw_36(vec![4u8; 36]),
            deletes_entry_address: EntryHash::from_raw_36(vec![5u8; 36]),
        })
    }

    fn delete_link_data() -> ActionData {
        ActionData::DeleteLink(DeleteLinkData {
            base_address: EntryHash::from_raw_36(vec![6u8; 36]).into(),
            link_add_address: ActionHash::from_raw_36(vec![7u8; 36]),
        })
    }

    #[test]
    fn store_entry_accepts_create_and_update() {
        let entry = crate::Entry::Agent(AgentPubKey::from_raw_36(vec![1u8; 36]));
        assert!(StoreEntry::new(signed_action(create_data()), entry.clone()).is_ok());

        let update = ActionData::Update(crate::dht_v2::UpdateData {
            original_action_address: ActionHash::from_raw_36(vec![10u8; 36]),
            original_entry_address: EntryHash::from_raw_36(vec![11u8; 36]),
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::from_raw_36(vec![12u8; 36]),
        });
        assert!(StoreEntry::new(signed_action(update), entry).is_ok());
    }

    #[test]
    fn store_entry_rejects_non_entry_action() {
        let entry = crate::Entry::Agent(AgentPubKey::from_raw_36(vec![1u8; 36]));
        assert!(StoreEntry::new(signed_action(delete_data()), entry).is_err());
    }

    #[test]
    fn register_delete_rejects_non_delete() {
        assert!(RegisterDelete::new(signed_action(create_data())).is_err());
        assert!(RegisterDelete::new(signed_action(delete_data())).is_ok());
    }

    #[test]
    fn register_delete_link_requires_delete_link_and_create_link() {
        let create_link = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_timestamp::Timestamp::from_micros(1),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::CreateLink(crate::dht_v2::CreateLinkData {
                base_address: EntryHash::from_raw_36(vec![6u8; 36]).into(),
                target_address: EntryHash::from_raw_36(vec![8u8; 36]).into(),
                zome_index: crate::action::ZomeIndex(0),
                link_type: crate::link::LinkType(0),
                tag: crate::link::LinkTag(vec![]),
            }),
        };
        assert!(RegisterDeleteLink::new(signed_action(delete_link_data()), create_link).is_ok());
    }

    #[test]
    fn register_delete_link_rejects_mismatched_base_address() {
        // create_link's base differs from delete_link_data()'s base.
        let create_link = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_timestamp::Timestamp::from_micros(1),
                action_seq: 0,
                prev_action: None,
            },
            data: ActionData::CreateLink(crate::dht_v2::CreateLinkData {
                base_address: EntryHash::from_raw_36(vec![9u8; 36]).into(),
                target_address: EntryHash::from_raw_36(vec![8u8; 36]).into(),
                zome_index: crate::action::ZomeIndex(0),
                link_type: crate::link::LinkType(0),
                tag: crate::link::LinkTag(vec![]),
            }),
        };
        assert!(RegisterDeleteLink::new(signed_action(delete_link_data()), create_link).is_err());
    }

    #[test]
    fn op_accessors_read_header_and_data() {
        let sah = signed_action(create_data());
        let expected_hash = sah.as_hash().clone();
        let op = Op::RegisterAgentActivity(RegisterAgentActivity {
            action: sah,
            cached_entry: None,
        });

        assert_eq!(op.action_seq(), 1);
        assert_eq!(op.author(), &AgentPubKey::from_raw_36(vec![1u8; 36]));
        assert_eq!(
            op.timestamp(),
            holochain_timestamp::Timestamp::from_micros(7)
        );
        assert_eq!(
            op.prev_action(),
            Some(&ActionHash::from_raw_36(vec![2u8; 36]))
        );
        assert_eq!(op.action_type(), crate::dht_v2::ActionType::Create);
        assert_eq!(op.action_hash(), &expected_hash);

        let (entry_hash, entry_type) = op.entry_data().expect("create has entry data");
        assert_eq!(entry_hash, &EntryHash::from_raw_36(vec![3u8; 36]));
        assert_eq!(entry_type, &EntryType::AgentPubKey);
    }

    #[test]
    fn op_entry_data_none_for_delete() {
        let op = Op::RegisterDelete(RegisterDelete::new(signed_action(delete_data())).unwrap());
        assert!(op.entry_data().is_none());
    }

    #[test]
    fn op_serde_roundtrip() {
        let entry = crate::Entry::Agent(AgentPubKey::from_raw_36(vec![1u8; 36]));
        let store_entry =
            Op::StoreEntry(StoreEntry::new(signed_action(create_data()), entry).unwrap());
        let store_record = Op::StoreRecord(StoreRecord {
            record: Record::new(signed_action(create_data()), crate::record::RecordEntry::NA),
        });
        for op in [store_entry, store_record] {
            let bytes = holochain_serialized_bytes::encode(&op).unwrap();
            let decoded: Op = holochain_serialized_bytes::decode(&bytes).unwrap();
            assert_eq!(decoded, op);
        }
    }

    #[test]
    fn op_accessors_work_through_store_record() {
        let sah = signed_action(create_data());
        let expected_hash = sah.as_hash().clone();
        let record = Record::new(sah, crate::record::RecordEntry::NA);
        let op = Op::StoreRecord(StoreRecord { record });
        assert_eq!(op.action_hash(), &expected_hash);
        assert_eq!(op.action_seq(), 1);
    }

    #[test]
    fn op_entry_data_some_for_update() {
        let update = ActionData::Update(crate::dht_v2::UpdateData {
            original_action_address: ActionHash::from_raw_36(vec![10u8; 36]),
            original_entry_address: EntryHash::from_raw_36(vec![11u8; 36]),
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::from_raw_36(vec![12u8; 36]),
        });
        let op = Op::RegisterUpdate(RegisterUpdate::new(signed_action(update), None).unwrap());
        let (entry_hash, entry_type) = op.entry_data().expect("update has entry data");
        assert_eq!(entry_hash, &EntryHash::from_raw_36(vec![12u8; 36]));
        assert_eq!(entry_type, &EntryType::AgentPubKey);
    }
}
