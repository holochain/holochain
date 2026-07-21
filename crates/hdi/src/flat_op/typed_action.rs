//! [`TypedAction`] pairs the header fields common to every action variant with action data
//! whose specific shape is already known from context — the `FlatOp`/`OpEntry`/`OpUpdate`/...
//! variant this value lives in already guarantees which [`ActionData`] case it holds.

use super::*;

/// Pairs the header fields common to every action variant with data whose precise shape
/// (`D`) is already known from which `FlatOp`/`OpEntry`/`OpUpdate`/... variant this value
/// lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedAction<D> {
    /// Header fields common to every action variant.
    pub header: ActionHeader,
    /// The precise, statically-known action data.
    pub data: D,
}

impl<D> TypedAction<D> {
    /// The public key of the agent who authored this action.
    pub fn author(&self) -> &AgentPubKey {
        &self.header.author
    }

    /// The microsecond timestamp at which this action was authored.
    pub fn timestamp(&self) -> Timestamp {
        self.header.timestamp
    }

    /// This action's position on the authoring agent's source chain.
    pub fn action_seq(&self) -> u32 {
        self.header.action_seq
    }

    /// The hash of the preceding action on the source chain. `None` only for the
    /// genesis `Dna` action.
    pub fn prev_action(&self) -> Option<&ActionHash> {
        self.header.prev_action.as_ref()
    }
}

impl TypedAction<CreateData> {
    /// The type of the entry this action creates.
    pub fn entry_type(&self) -> &EntryType {
        &self.data.entry_type
    }

    /// The hash of the entry this action creates.
    pub fn entry_hash(&self) -> &EntryHash {
        &self.data.entry_hash
    }
}

impl TypedAction<UpdateData> {
    /// The type of the entry this action creates.
    pub fn entry_type(&self) -> &EntryType {
        &self.data.entry_type
    }

    /// The hash of the entry this action creates.
    pub fn entry_hash(&self) -> &EntryHash {
        &self.data.entry_hash
    }
}

/// The action data for an action known to create or update an entry — the original action
/// referenced when validating an `Update` or `Delete`, which is always a `Create` or an
/// `Update`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryCreationData {
    /// The original action created the entry.
    Create(CreateData),
    /// The original action updated the entry.
    Update(UpdateData),
}

impl EntryCreationData {
    /// The type of the entry this action references.
    pub fn entry_type(&self) -> &EntryType {
        match self {
            EntryCreationData::Create(d) => &d.entry_type,
            EntryCreationData::Update(d) => &d.entry_type,
        }
    }

    /// The hash of the entry this action references.
    pub fn entry_hash(&self) -> &EntryHash {
        match self {
            EntryCreationData::Create(d) => &d.entry_hash,
            EntryCreationData::Update(d) => &d.entry_hash,
        }
    }
}

impl TypedAction<EntryCreationData> {
    /// The type of the entry this action references.
    pub fn entry_type(&self) -> &EntryType {
        self.data.entry_type()
    }

    /// The hash of the entry this action references.
    pub fn entry_hash(&self) -> &EntryHash {
        self.data.entry_hash()
    }
}

impl TryFrom<Action> for TypedAction<EntryCreationData> {
    type Error = WrongActionError;

    /// Narrows a freshly-fetched [`Action`] (e.g. from `must_get_action`) down to the
    /// entry-creation case, erroring if it's neither `Create` nor `Update`.
    fn try_from(action: Action) -> Result<Self, Self::Error> {
        let data = match action.data {
            ActionData::Create(d) => EntryCreationData::Create(d),
            ActionData::Update(d) => EntryCreationData::Update(d),
            other => {
                return Err(WrongActionError(format!(
                    "Expected a Create or Update action, got {:?}",
                    other.action_type()
                )))
            }
        };
        Ok(TypedAction {
            header: action.header,
            data,
        })
    }
}

/// Data that can be embedded back into an [`ActionData`] — the inverse of the narrowing
/// that produces a `TypedAction<D>` from a full [`Action`]. Lets any `TypedAction<D>` be
/// converted back into a full `Action` via `.into()`, for APIs (like `hash_action`) that
/// still need the generic type.
pub trait IntoActionData {
    /// Embed this data back into the matching [`ActionData`] variant.
    fn into_action_data(self) -> ActionData;
}

impl IntoActionData for CreateData {
    fn into_action_data(self) -> ActionData {
        ActionData::Create(self)
    }
}

impl IntoActionData for UpdateData {
    fn into_action_data(self) -> ActionData {
        ActionData::Update(self)
    }
}

impl IntoActionData for DeleteData {
    fn into_action_data(self) -> ActionData {
        ActionData::Delete(self)
    }
}

impl IntoActionData for CreateLinkData {
    fn into_action_data(self) -> ActionData {
        ActionData::CreateLink(self)
    }
}

impl IntoActionData for DeleteLinkData {
    fn into_action_data(self) -> ActionData {
        ActionData::DeleteLink(self)
    }
}

impl IntoActionData for EntryCreationData {
    fn into_action_data(self) -> ActionData {
        match self {
            EntryCreationData::Create(d) => ActionData::Create(d),
            EntryCreationData::Update(d) => ActionData::Update(d),
        }
    }
}

impl<D: IntoActionData> From<TypedAction<D>> for Action {
    fn from(typed: TypedAction<D>) -> Self {
        Action {
            header: typed.header,
            data: typed.data.into_action_data(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::short_hand::{ah, ak, eh, public_app_entry_def};
    use holochain_integrity_types::action::DeleteData;

    fn header() -> ActionHeader {
        ActionHeader {
            author: ak(1),
            timestamp: Timestamp::from_micros(42),
            action_seq: 3,
            prev_action: Some(ah(2)),
        }
    }

    fn create_data() -> CreateData {
        CreateData {
            entry_type: EntryType::App(public_app_entry_def(0, 0)),
            entry_hash: eh(3),
        }
    }

    fn update_data() -> UpdateData {
        UpdateData {
            original_action_address: ah(4),
            original_entry_address: eh(5),
            entry_type: EntryType::App(public_app_entry_def(0, 0)),
            entry_hash: eh(6),
        }
    }

    #[test]
    fn typed_action_exposes_header_fields() {
        let action = TypedAction {
            header: header(),
            data: create_data(),
        };
        assert_eq!(action.author(), &ak(1));
        assert_eq!(action.timestamp(), Timestamp::from_micros(42));
        assert_eq!(action.action_seq(), 3);
        assert_eq!(action.prev_action(), Some(&ah(2)));
    }

    #[test]
    fn typed_action_create_exposes_entry_fields() {
        let action = TypedAction {
            header: header(),
            data: create_data(),
        };
        assert_eq!(
            action.entry_type(),
            &EntryType::App(public_app_entry_def(0, 0))
        );
        assert_eq!(action.entry_hash(), &eh(3));
    }

    #[test]
    fn entry_creation_data_forwards_create_and_update_fields() {
        let create = EntryCreationData::Create(create_data());
        assert_eq!(create.entry_hash(), &eh(3));
        let update = EntryCreationData::Update(update_data());
        assert_eq!(update.entry_hash(), &eh(6));
    }

    #[test]
    fn try_from_action_succeeds_for_create_and_update() {
        let create_action = Action {
            header: header(),
            data: ActionData::Create(create_data()),
        };
        let typed = TypedAction::<EntryCreationData>::try_from(create_action).unwrap();
        assert!(matches!(typed.data, EntryCreationData::Create(_)));

        let update_action = Action {
            header: header(),
            data: ActionData::Update(update_data()),
        };
        let typed = TypedAction::<EntryCreationData>::try_from(update_action).unwrap();
        assert!(matches!(typed.data, EntryCreationData::Update(_)));
    }

    #[test]
    fn try_from_action_fails_for_delete() {
        let action = Action {
            header: header(),
            data: ActionData::Delete(DeleteData {
                deletes_address: ah(7),
                deletes_entry_address: eh(8),
            }),
        };
        assert!(TypedAction::<EntryCreationData>::try_from(action).is_err());
    }

    #[test]
    fn typed_action_round_trips_into_action() {
        let typed = TypedAction {
            header: header(),
            data: create_data(),
        };
        let action: Action = typed.clone().into();
        assert_eq!(action.header, typed.header);
        assert_eq!(action.data, ActionData::Create(typed.data));
    }
}
