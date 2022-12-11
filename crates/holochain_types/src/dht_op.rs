//! Data structures representing the operations that can be performed within a Holochain DHT.
//!
//! See the [item-level documentation for `DhtOp`][DhtOp] for more details.
//!
//! [DhtOp]: enum.DhtOp.html

use std::str::FromStr;

use crate::action::NewEntryAction;
use crate::prelude::*;
use crate::record::RecordGroup;
use error::DhtOpError;
use error::DhtOpResult;
use holo_hash::*;
use holochain_sqlite::rusqlite::types::FromSql;
use holochain_sqlite::rusqlite::ToSql;
use holochain_zome_types::action;
use holochain_zome_types::prelude::*;
use kitsune_p2p_dht::region::RegionData;
use kitsune_p2p_dht::Loc;
use serde::Deserialize;
use serde::Serialize;

#[allow(missing_docs)]
pub mod error;

#[cfg(test)]
pub mod tests;

#[cfg(feature = "test_utils")]
pub mod facts;

/// A unit of DHT gossip. Used to notify an authority of new (meta)data to hold
/// as well as changes to the status of already held data.
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, derive_more::Display,
)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub enum DhtOp {
    #[display(fmt = "StoreRecord")]
    /// Used to notify the authority for an action that it has been created.
    ///
    /// Conceptually, authorities receiving this `DhtOp` do three things:
    ///
    /// - Ensure that the record passes validation.
    /// - Store the action into their DHT shard.
    /// - Store the entry into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that entry up-to-date.
    StoreRecord(Signature, Action, Option<Box<Entry>>),

    #[display(fmt = "StoreEntry")]
    /// Used to notify the authority for an entry that it has been created
    /// anew. (The same entry can be created more than once.)
    ///
    /// Conceptually, authorities receiving this `DhtOp` do four things:
    ///
    /// - Ensure that the record passes validation.
    /// - Store the entry into their DHT shard.
    /// - Store the action into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that action up-to-date.
    /// - Add a "created-by" reference from the entry to the hash of the action.
    ///
    /// TODO: document how those "created-by" references are stored in
    /// reality.
    StoreEntry(Signature, NewEntryAction, Box<Entry>),

    #[display(fmt = "RegisterAgentActivity")]
    /// Used to notify the authority for an agent's public key that that agent
    /// has committed a new action.
    ///
    /// Conceptually, authorities receiving this `DhtOp` do three things:
    ///
    /// - Ensure that *the action alone* passes surface-level validation.
    /// - Store the action into their DHT shard.
    ///   - FIXME: @artbrock, do they?
    /// - Add an "agent-activity" reference from the public key to the hash
    ///   of the action.
    ///
    /// TODO: document how those "agent-activity" references are stored in
    /// reality.
    RegisterAgentActivity(Signature, Action),

    #[display(fmt = "RegisterUpdatedContent")]
    /// Op for updating an entry.
    /// This is sent to the entry authority.
    // TODO: This entry is here for validation by the entry update action holder
    // link's don't do this. The entry is validated by store entry. Maybe we either
    // need to remove the Entry here or add it to link.
    RegisterUpdatedContent(Signature, action::Update, Option<Box<Entry>>),

    #[display(fmt = "RegisterUpdatedRecord")]
    /// Op for updating a record.
    /// This is sent to the record authority.
    RegisterUpdatedRecord(Signature, action::Update, Option<Box<Entry>>),

    #[display(fmt = "RegisterDeletedBy")]
    /// Op for registering an action deletion with the Action authority
    RegisterDeletedBy(Signature, action::Delete),

    #[display(fmt = "RegisterDeletedEntryAction")]
    /// Op for registering an action deletion with the Entry authority, so that
    /// the Entry can be marked Dead if all of its Actions have been deleted
    RegisterDeletedEntryAction(Signature, action::Delete),

    #[display(fmt = "RegisterAddLink")]
    /// Op for adding a link
    RegisterAddLink(Signature, action::CreateLink),

    #[display(fmt = "RegisterRemoveLink")]
    /// Op for removing a link
    RegisterRemoveLink(Signature, action::DeleteLink),
}

impl kitsune_p2p_dht::prelude::OpRegion for DhtOp {
    fn loc(&self) -> Loc {
        self.dht_basis().get_loc()
    }

    fn timestamp(&self) -> Timestamp {
        self.timestamp()
    }

    fn region_data(&self) -> RegionData {
        unimplemented!()
    }

    fn bound(_timestamp: Timestamp, _loc: kitsune_p2p_dht::Loc) -> Self {
        unimplemented!()
    }
}

/// A type for storing in databases that don't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, derive_more::Display)]
pub enum DhtOpLight {
    #[display(fmt = "StoreRecord")]
    StoreRecord(ActionHash, Option<EntryHash>, OpBasis),
    #[display(fmt = "StoreEntry")]
    StoreEntry(ActionHash, EntryHash, OpBasis),
    #[display(fmt = "RegisterAgentActivity")]
    RegisterAgentActivity(ActionHash, OpBasis),
    #[display(fmt = "RegisterUpdatedContent")]
    RegisterUpdatedContent(ActionHash, EntryHash, OpBasis),
    #[display(fmt = "RegisterUpdatedRecord")]
    RegisterUpdatedRecord(ActionHash, EntryHash, OpBasis),
    #[display(fmt = "RegisterDeletedBy")]
    RegisterDeletedBy(ActionHash, OpBasis),
    #[display(fmt = "RegisterDeletedEntryAction")]
    RegisterDeletedEntryAction(ActionHash, OpBasis),
    #[display(fmt = "RegisterAddLink")]
    RegisterAddLink(ActionHash, OpBasis),
    #[display(fmt = "RegisterRemoveLink")]
    RegisterRemoveLink(ActionHash, OpBasis),
}

impl PartialEq for DhtOpLight {
    fn eq(&self, other: &Self) -> bool {
        // The ops are the same if they are the same type on the same action hash.
        // We can't derive eq because `Option<EntryHash>` doesn't make the op different.
        // We can ignore the basis because the basis is derived from the action and op type.
        self.get_type() == other.get_type() && self.action_hash() == other.action_hash()
    }
}

impl Eq for DhtOpLight {}

impl std::hash::Hash for DhtOpLight {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_type().hash(state);
        self.action_hash().hash(state);
    }
}

/// This enum is used to
#[allow(missing_docs)]
#[derive(
    Clone,
    Copy,
    Debug,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    derive_more::Display,
    strum_macros::EnumString,
)]
pub enum DhtOpType {
    #[display(fmt = "StoreRecord")]
    StoreRecord,
    #[display(fmt = "StoreEntry")]
    StoreEntry,
    #[display(fmt = "RegisterAgentActivity")]
    RegisterAgentActivity,
    #[display(fmt = "RegisterUpdatedContent")]
    RegisterUpdatedContent,
    #[display(fmt = "RegisterUpdatedRecord")]
    RegisterUpdatedRecord,
    #[display(fmt = "RegisterDeletedBy")]
    RegisterDeletedBy,
    #[display(fmt = "RegisterDeletedEntryAction")]
    RegisterDeletedEntryAction,
    #[display(fmt = "RegisterAddLink")]
    RegisterAddLink,
    #[display(fmt = "RegisterRemoveLink")]
    RegisterRemoveLink,
}

impl ToSql for DhtOpType {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            format!("{}", self).into(),
        ))
    }
}

impl FromSql for DhtOpType {
    fn column_result(
        value: holochain_sqlite::rusqlite::types::ValueRef<'_>,
    ) -> holochain_sqlite::rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).and_then(|string| {
            DhtOpType::from_str(&string)
                .map_err(|_| holochain_sqlite::rusqlite::types::FromSqlError::InvalidType)
        })
    }
}

impl DhtOp {
    fn as_unique_form(&self) -> UniqueForm<'_> {
        match self {
            Self::StoreRecord(_, action, _) => UniqueForm::StoreRecord(action),
            Self::StoreEntry(_, action, _) => UniqueForm::StoreEntry(action),
            Self::RegisterAgentActivity(_, action) => UniqueForm::RegisterAgentActivity(action),
            Self::RegisterUpdatedContent(_, action, _) => {
                UniqueForm::RegisterUpdatedContent(action)
            }
            Self::RegisterUpdatedRecord(_, action, _) => UniqueForm::RegisterUpdatedRecord(action),
            Self::RegisterDeletedBy(_, action) => UniqueForm::RegisterDeletedBy(action),
            Self::RegisterDeletedEntryAction(_, action) => {
                UniqueForm::RegisterDeletedEntryAction(action)
            }
            Self::RegisterAddLink(_, action) => UniqueForm::RegisterAddLink(action),
            Self::RegisterRemoveLink(_, action) => UniqueForm::RegisterRemoveLink(action),
        }
    }

    /// Returns the basis hash which determines which agents will receive this DhtOp
    pub fn dht_basis(&self) -> OpBasis {
        self.as_unique_form().basis()
    }

    /// Convert a [DhtOp] to a [DhtOpLight] and basis
    pub fn to_light(
        // Hoping one day we can work out how to go from `&Create`
        // to `&Action::Create(Create)` so punting on a reference
        &self,
    ) -> DhtOpLight {
        let basis = self.dht_basis();
        match self {
            DhtOp::StoreRecord(_, a, _) => {
                let e = a.entry_data().map(|(e, _)| e.clone());
                let h = ActionHash::with_data_sync(a);
                DhtOpLight::StoreRecord(h, e, basis)
            }
            DhtOp::StoreEntry(_, a, _) => {
                let e = a.entry().clone();
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::StoreEntry(h, e, basis)
            }
            DhtOp::RegisterAgentActivity(_, a) => {
                let h = ActionHash::with_data_sync(a);
                DhtOpLight::RegisterAgentActivity(h, basis)
            }
            DhtOp::RegisterUpdatedContent(_, a, _) => {
                let e = a.entry_hash.clone();
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::RegisterUpdatedContent(h, e, basis)
            }
            DhtOp::RegisterUpdatedRecord(_, a, _) => {
                let e = a.entry_hash.clone();
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::RegisterUpdatedRecord(h, e, basis)
            }
            DhtOp::RegisterDeletedBy(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::RegisterDeletedBy(h, basis)
            }
            DhtOp::RegisterDeletedEntryAction(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::RegisterDeletedEntryAction(h, basis)
            }
            DhtOp::RegisterAddLink(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::RegisterAddLink(h, basis)
            }
            DhtOp::RegisterRemoveLink(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                DhtOpLight::RegisterRemoveLink(h, basis)
            }
        }
    }

    /// Get the signature for this op
    pub fn signature(&self) -> &Signature {
        match self {
            DhtOp::StoreRecord(s, _, _)
            | DhtOp::StoreEntry(s, _, _)
            | DhtOp::RegisterAgentActivity(s, _)
            | DhtOp::RegisterUpdatedContent(s, _, _)
            | DhtOp::RegisterUpdatedRecord(s, _, _)
            | DhtOp::RegisterDeletedBy(s, _)
            | DhtOp::RegisterDeletedEntryAction(s, _)
            | DhtOp::RegisterAddLink(s, _)
            | DhtOp::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Extract inner Signature, Action and Option<Entry> from an op
    pub fn into_inner(self) -> (Signature, Action, Option<Entry>) {
        match self {
            DhtOp::StoreRecord(s, h, e) => (s, h, e.map(|e| *e)),
            DhtOp::StoreEntry(s, h, e) => (s, h.into(), Some(*e)),
            DhtOp::RegisterAgentActivity(s, h) => (s, h, None),
            DhtOp::RegisterUpdatedContent(s, h, e) => (s, h.into(), e.map(|e| *e)),
            DhtOp::RegisterUpdatedRecord(s, h, e) => (s, h.into(), e.map(|e| *e)),
            DhtOp::RegisterDeletedBy(s, h) => (s, h.into(), None),
            DhtOp::RegisterDeletedEntryAction(s, h) => (s, h.into(), None),
            DhtOp::RegisterAddLink(s, h) => (s, h.into(), None),
            DhtOp::RegisterRemoveLink(s, h) => (s, h.into(), None),
        }
    }

    /// Get the action from this op
    /// This requires cloning and converting the action
    /// as some ops don't hold the Action type
    pub fn action(&self) -> Action {
        match self {
            DhtOp::StoreRecord(_, h, _) => h.clone(),
            DhtOp::StoreEntry(_, h, _) => h.clone().into(),
            DhtOp::RegisterAgentActivity(_, h) => h.clone(),
            DhtOp::RegisterUpdatedContent(_, h, _) => h.clone().into(),
            DhtOp::RegisterUpdatedRecord(_, h, _) => h.clone().into(),
            DhtOp::RegisterDeletedBy(_, h) => h.clone().into(),
            DhtOp::RegisterDeletedEntryAction(_, h) => h.clone().into(),
            DhtOp::RegisterAddLink(_, h) => h.clone().into(),
            DhtOp::RegisterRemoveLink(_, h) => h.clone().into(),
        }
    }

    /// Get the entry from this op, if one exists
    pub fn entry(&self) -> Option<&Entry> {
        match self {
            DhtOp::StoreRecord(_, _, e) => e.as_ref().map(|b| &**b),
            DhtOp::StoreEntry(_, _, e) => Some(&*e),
            DhtOp::RegisterUpdatedContent(_, _, e) => e.as_ref().map(|b| &**b),
            DhtOp::RegisterUpdatedRecord(_, _, e) => e.as_ref().map(|b| &**b),
            DhtOp::RegisterAgentActivity(_, _) => None,
            DhtOp::RegisterDeletedBy(_, _) => None,
            DhtOp::RegisterDeletedEntryAction(_, _) => None,
            DhtOp::RegisterAddLink(_, _) => None,
            DhtOp::RegisterRemoveLink(_, _) => None,
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> DhtOpType {
        match self {
            DhtOp::StoreRecord(_, _, _) => DhtOpType::StoreRecord,
            DhtOp::StoreEntry(_, _, _) => DhtOpType::StoreEntry,
            DhtOp::RegisterUpdatedContent(_, _, _) => DhtOpType::RegisterUpdatedContent,
            DhtOp::RegisterUpdatedRecord(_, _, _) => DhtOpType::RegisterUpdatedRecord,
            DhtOp::RegisterAgentActivity(_, _) => DhtOpType::RegisterAgentActivity,
            DhtOp::RegisterDeletedBy(_, _) => DhtOpType::RegisterDeletedBy,
            DhtOp::RegisterDeletedEntryAction(_, _) => DhtOpType::RegisterDeletedEntryAction,
            DhtOp::RegisterAddLink(_, _) => DhtOpType::RegisterAddLink,
            DhtOp::RegisterRemoveLink(_, _) => DhtOpType::RegisterRemoveLink,
        }
    }

    /// From a type, action and an entry (if there is one)
    pub fn from_type(
        op_type: DhtOpType,
        action: SignedAction,
        entry: Option<Entry>,
    ) -> DhtOpResult<Self> {
        let SignedAction(action, signature) = action;
        let r = match op_type {
            DhtOpType::StoreRecord => DhtOp::StoreRecord(signature, action, entry.map(Box::new)),
            DhtOpType::StoreEntry => {
                let entry = entry.ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                let action = match action {
                    Action::Create(c) => NewEntryAction::Create(c),
                    Action::Update(c) => NewEntryAction::Update(c),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                DhtOp::StoreEntry(signature, action, Box::new(entry))
            }
            DhtOpType::RegisterAgentActivity => DhtOp::RegisterAgentActivity(signature, action),
            DhtOpType::RegisterUpdatedContent => {
                DhtOp::RegisterUpdatedContent(signature, action.try_into()?, entry.map(Box::new))
            }
            DhtOpType::RegisterUpdatedRecord => {
                DhtOp::RegisterUpdatedRecord(signature, action.try_into()?, entry.map(Box::new))
            }
            DhtOpType::RegisterDeletedBy => DhtOp::RegisterDeletedBy(signature, action.try_into()?),
            DhtOpType::RegisterDeletedEntryAction => {
                DhtOp::RegisterDeletedEntryAction(signature, action.try_into()?)
            }
            DhtOpType::RegisterAddLink => DhtOp::RegisterAddLink(signature, action.try_into()?),
            DhtOpType::RegisterRemoveLink => {
                DhtOp::RegisterRemoveLink(signature, action.try_into()?)
            }
        };
        Ok(r)
    }

    fn to_order(&self) -> OpOrder {
        OpOrder::new(self.get_type(), self.timestamp())
    }

    /// Enzymatic countersigning session ops need special handling so that they
    /// arrive at the enzyme and not elsewhere. If this isn't an enzymatic
    /// countersigning session then the return will be None so can be used as
    /// a boolean for filtering with is_some().
    pub fn enzymatic_countersigning_enzyme(&self) -> Option<&AgentPubKey> {
        if let Some(Entry::CounterSign(session_data, _)) = self.entry() {
            if session_data.preflight_request().enzymatic {
                session_data
                    .preflight_request()
                    .signing_agents
                    .get(0)
                    .map(|(pubkey, _)| pubkey)
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl PartialOrd for DhtOp {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DhtOp {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_order().cmp(&other.to_order())
    }
}

impl DhtOpLight {
    /// Get the dht basis for where to send this op
    pub fn dht_basis(&self) -> &OpBasis {
        match self {
            DhtOpLight::StoreRecord(_, _, b)
            | DhtOpLight::StoreEntry(_, _, b)
            | DhtOpLight::RegisterAgentActivity(_, b)
            | DhtOpLight::RegisterUpdatedContent(_, _, b)
            | DhtOpLight::RegisterUpdatedRecord(_, _, b)
            | DhtOpLight::RegisterDeletedBy(_, b)
            | DhtOpLight::RegisterDeletedEntryAction(_, b)
            | DhtOpLight::RegisterAddLink(_, b)
            | DhtOpLight::RegisterRemoveLink(_, b) => b,
        }
    }
    /// Get the action hash from this op
    pub fn action_hash(&self) -> &ActionHash {
        match self {
            DhtOpLight::StoreRecord(h, _, _)
            | DhtOpLight::StoreEntry(h, _, _)
            | DhtOpLight::RegisterAgentActivity(h, _)
            | DhtOpLight::RegisterUpdatedContent(h, _, _)
            | DhtOpLight::RegisterUpdatedRecord(h, _, _)
            | DhtOpLight::RegisterDeletedBy(h, _)
            | DhtOpLight::RegisterDeletedEntryAction(h, _)
            | DhtOpLight::RegisterAddLink(h, _)
            | DhtOpLight::RegisterRemoveLink(h, _) => h,
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> DhtOpType {
        match self {
            DhtOpLight::StoreRecord(_, _, _) => DhtOpType::StoreRecord,
            DhtOpLight::StoreEntry(_, _, _) => DhtOpType::StoreEntry,
            DhtOpLight::RegisterUpdatedContent(_, _, _) => DhtOpType::RegisterUpdatedContent,
            DhtOpLight::RegisterUpdatedRecord(_, _, _) => DhtOpType::RegisterUpdatedRecord,
            DhtOpLight::RegisterAgentActivity(_, _) => DhtOpType::RegisterAgentActivity,
            DhtOpLight::RegisterDeletedBy(_, _) => DhtOpType::RegisterDeletedBy,
            DhtOpLight::RegisterDeletedEntryAction(_, _) => DhtOpType::RegisterDeletedEntryAction,
            DhtOpLight::RegisterAddLink(_, _) => DhtOpType::RegisterAddLink,
            DhtOpLight::RegisterRemoveLink(_, _) => DhtOpType::RegisterRemoveLink,
        }
    }

    /// From a type with the hashes.
    pub fn from_type(
        op_type: DhtOpType,
        action_hash: ActionHash,
        action: &Action,
    ) -> DhtOpResult<Self> {
        let op = match op_type {
            DhtOpType::StoreRecord => {
                let entry_hash = action.entry_hash().cloned();
                Self::StoreRecord(action_hash.clone(), entry_hash, action_hash.into())
            }
            DhtOpType::StoreEntry => {
                let entry_hash = action
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                Self::StoreEntry(action_hash, entry_hash.clone(), entry_hash.into())
            }
            DhtOpType::RegisterAgentActivity => {
                Self::RegisterAgentActivity(action_hash, action.author().clone().into())
            }
            DhtOpType::RegisterUpdatedContent => {
                let entry_hash = action
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                let basis = match action {
                    Action::Update(update) => update.original_entry_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterUpdatedContent(action_hash, entry_hash, basis.into())
            }
            DhtOpType::RegisterUpdatedRecord => {
                let entry_hash = action
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                let basis = match action {
                    Action::Update(update) => update.original_entry_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterUpdatedRecord(action_hash, entry_hash, basis.into())
            }
            DhtOpType::RegisterDeletedBy => {
                let basis = match action {
                    Action::Delete(delete) => delete.deletes_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterDeletedBy(action_hash, basis.into())
            }
            DhtOpType::RegisterDeletedEntryAction => {
                let basis = match action {
                    Action::Delete(delete) => delete.deletes_entry_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterDeletedEntryAction(action_hash, basis.into())
            }
            DhtOpType::RegisterAddLink => {
                let basis = match action {
                    Action::CreateLink(create_link) => create_link.base_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterAddLink(action_hash, basis)
            }
            DhtOpType::RegisterRemoveLink => {
                let basis = match action {
                    Action::DeleteLink(delete_link) => delete_link.base_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterRemoveLink(action_hash, basis)
            }
        };
        Ok(op)
    }
}

#[allow(missing_docs)]
#[derive(Serialize, Debug)]
pub enum UniqueForm<'a> {
    // As an optimization, we don't include signatures. They would be redundant
    // with actions and therefore would waste hash/comparison time to include.
    StoreRecord(&'a Action),
    StoreEntry(&'a NewEntryAction),
    RegisterAgentActivity(&'a Action),
    RegisterUpdatedContent(&'a action::Update),
    RegisterUpdatedRecord(&'a action::Update),
    RegisterDeletedBy(&'a action::Delete),
    RegisterDeletedEntryAction(&'a action::Delete),
    RegisterAddLink(&'a action::CreateLink),
    RegisterRemoveLink(&'a action::DeleteLink),
}

impl<'a> UniqueForm<'a> {
    fn basis(&'a self) -> OpBasis {
        match self {
            UniqueForm::StoreRecord(action) => ActionHash::with_data_sync(*action).into(),
            UniqueForm::StoreEntry(action) => action.entry().clone().into(),
            UniqueForm::RegisterAgentActivity(action) => action.author().clone().into(),
            UniqueForm::RegisterUpdatedContent(action) => {
                action.original_entry_address.clone().into()
            }
            UniqueForm::RegisterUpdatedRecord(action) => {
                action.original_action_address.clone().into()
            }
            UniqueForm::RegisterDeletedBy(action) => action.deletes_address.clone().into(),
            UniqueForm::RegisterDeletedEntryAction(action) => {
                action.deletes_entry_address.clone().into()
            }
            UniqueForm::RegisterAddLink(action) => action.base_address.clone(),
            UniqueForm::RegisterRemoveLink(action) => action.base_address.clone(),
        }
    }

    /// Get the dht op hash without cloning the action.
    pub fn op_hash(op_type: DhtOpType, action: Action) -> DhtOpResult<(Action, DhtOpHash)> {
        match op_type {
            DhtOpType::StoreRecord => {
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreRecord(&action));
                Ok((action, hash))
            }
            DhtOpType::StoreEntry => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreEntry(&action));
                Ok((action.into(), hash))
            }
            DhtOpType::RegisterAgentActivity => {
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAgentActivity(&action));
                Ok((action, hash))
            }
            DhtOpType::RegisterUpdatedContent => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterUpdatedContent(&action));
                Ok((action.into(), hash))
            }
            DhtOpType::RegisterUpdatedRecord => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterUpdatedRecord(&action));
                Ok((action.into(), hash))
            }
            DhtOpType::RegisterDeletedBy => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterDeletedBy(&action));
                Ok((action.into(), hash))
            }
            DhtOpType::RegisterDeletedEntryAction => {
                let action = action.try_into()?;
                let hash =
                    DhtOpHash::with_data_sync(&UniqueForm::RegisterDeletedEntryAction(&action));
                Ok((action.into(), hash))
            }
            DhtOpType::RegisterAddLink => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAddLink(&action));
                Ok((action.into(), hash))
            }
            DhtOpType::RegisterRemoveLink => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterRemoveLink(&action));
                Ok((action.into(), hash))
            }
        }
    }
}

/// Produce all DhtOps for a Record
pub fn produce_ops_from_record(record: &Record) -> DhtOpResult<Vec<DhtOp>> {
    let op_lights = produce_op_lights_from_records(vec![record])?;
    let (shh, maybe_entry) = record.clone().into_inner();
    let SignedActionHashed {
        hashed: ActionHashed {
            content: action, ..
        },
        signature,
    } = shh;

    let mut ops = Vec::with_capacity(op_lights.len());

    for op_light in op_lights {
        let signature = signature.clone();
        let action = action.clone();
        let op = match op_light {
            DhtOpLight::StoreRecord(_, _, _) => {
                let maybe_entry_box = maybe_entry.clone().into_option().map(Box::new);
                DhtOp::StoreRecord(signature, action, maybe_entry_box)
            }
            DhtOpLight::StoreEntry(_, _, _) => {
                let new_entry_action = action.clone().try_into()?;
                let box_entry = match maybe_entry.clone().into_option() {
                    Some(entry) => Box::new(entry),
                    None => {
                        // Entry is private so continue
                        continue;
                    }
                };
                DhtOp::StoreEntry(signature, new_entry_action, box_entry)
            }
            DhtOpLight::RegisterAgentActivity(_, _) => {
                DhtOp::RegisterAgentActivity(signature, action)
            }
            DhtOpLight::RegisterUpdatedContent(_, _, _) => {
                let entry_update = action.try_into()?;
                let maybe_entry_box = maybe_entry.clone().into_option().map(Box::new);
                DhtOp::RegisterUpdatedContent(signature, entry_update, maybe_entry_box)
            }
            DhtOpLight::RegisterUpdatedRecord(_, _, _) => {
                let entry_update = action.try_into()?;
                let maybe_entry_box = maybe_entry.clone().into_option().map(Box::new);
                DhtOp::RegisterUpdatedRecord(signature, entry_update, maybe_entry_box)
            }
            DhtOpLight::RegisterDeletedEntryAction(_, _) => {
                let record_delete = action.try_into()?;
                DhtOp::RegisterDeletedEntryAction(signature, record_delete)
            }
            DhtOpLight::RegisterDeletedBy(_, _) => {
                let record_delete = action.try_into()?;
                DhtOp::RegisterDeletedBy(signature, record_delete)
            }
            DhtOpLight::RegisterAddLink(_, _) => {
                let link_add = action.try_into()?;
                DhtOp::RegisterAddLink(signature, link_add)
            }
            DhtOpLight::RegisterRemoveLink(_, _) => {
                let link_remove = action.try_into()?;
                DhtOp::RegisterRemoveLink(signature, link_remove)
            }
        };
        ops.push(op);
    }
    Ok(ops)
}

/// Produce all the op lights for tese records
pub fn produce_op_lights_from_records(actions: Vec<&Record>) -> DhtOpResult<Vec<DhtOpLight>> {
    let actions_and_hashes = actions.into_iter().map(|e| {
        (
            e.action_address(),
            e.action(),
            e.action().entry_data().map(|(h, _)| h.clone()),
        )
    });
    produce_op_lights_from_iter(actions_and_hashes)
}

/// Produce all the op lights from this record group
/// with a shared entry
pub fn produce_op_lights_from_record_group(
    records: &RecordGroup<'_>,
) -> DhtOpResult<Vec<DhtOpLight>> {
    let actions_and_hashes = records.actions_and_hashes();
    let maybe_entry_hash = Some(records.entry_hash());
    produce_op_lights_from_parts(actions_and_hashes, maybe_entry_hash)
}

/// Data minimal clone (no cloning entries) cheap &Record to DhtOpLight conversion
fn produce_op_lights_from_parts<'a>(
    actions_and_hashes: impl Iterator<Item = (&'a ActionHash, &'a Action)>,
    maybe_entry_hash: Option<&EntryHash>,
) -> DhtOpResult<Vec<DhtOpLight>> {
    let iter = actions_and_hashes.map(|(head, hash)| (head, hash, maybe_entry_hash.cloned()));
    produce_op_lights_from_iter(iter)
}

/// Produce op lights from iter of (action hash, action, maybe entry).
pub fn produce_op_lights_from_iter<'a>(
    iter: impl Iterator<Item = (&'a ActionHash, &'a Action, Option<EntryHash>)>,
) -> DhtOpResult<Vec<DhtOpLight>> {
    let mut ops = Vec::new();

    for (action_hash, action, maybe_entry_hash) in iter {
        let op_lights = action_to_op_types(action)
            .into_iter()
            .filter_map(|op_type| {
                let op_light = match (op_type, action) {
                    (DhtOpType::StoreRecord, _) => {
                        let store_record_basis = UniqueForm::StoreRecord(action).basis();
                        DhtOpLight::StoreRecord(
                            action_hash.clone(),
                            maybe_entry_hash.clone(),
                            store_record_basis,
                        )
                    }
                    (DhtOpType::RegisterAgentActivity, _) => {
                        let register_activity_basis =
                            UniqueForm::RegisterAgentActivity(action).basis();
                        DhtOpLight::RegisterAgentActivity(
                            action_hash.clone(),
                            register_activity_basis,
                        )
                    }
                    (DhtOpType::StoreEntry, Action::Create(create)) => DhtOpLight::StoreEntry(
                        action_hash.clone(),
                        maybe_entry_hash.clone()?,
                        UniqueForm::StoreEntry(&NewEntryAction::Create(create.clone())).basis(),
                    ),
                    (DhtOpType::StoreEntry, Action::Update(update)) => DhtOpLight::StoreEntry(
                        action_hash.clone(),
                        maybe_entry_hash.clone()?,
                        UniqueForm::StoreEntry(&NewEntryAction::Update(update.clone())).basis(),
                    ),
                    (DhtOpType::RegisterUpdatedContent, Action::Update(update)) => {
                        DhtOpLight::RegisterUpdatedContent(
                            action_hash.clone(),
                            maybe_entry_hash.clone()?,
                            UniqueForm::RegisterUpdatedContent(update).basis(),
                        )
                    }
                    (DhtOpType::RegisterUpdatedRecord, Action::Update(update)) => {
                        DhtOpLight::RegisterUpdatedRecord(
                            action_hash.clone(),
                            maybe_entry_hash.clone()?,
                            UniqueForm::RegisterUpdatedRecord(update).basis(),
                        )
                    }
                    (DhtOpType::RegisterDeletedBy, Action::Delete(delete)) => {
                        DhtOpLight::RegisterDeletedBy(
                            action_hash.clone(),
                            UniqueForm::RegisterDeletedBy(delete).basis(),
                        )
                    }
                    (DhtOpType::RegisterDeletedEntryAction, Action::Delete(delete)) => {
                        DhtOpLight::RegisterDeletedEntryAction(
                            action_hash.clone(),
                            UniqueForm::RegisterDeletedEntryAction(delete).basis(),
                        )
                    }
                    (DhtOpType::RegisterAddLink, Action::CreateLink(create_link)) => {
                        DhtOpLight::RegisterAddLink(
                            action_hash.clone(),
                            UniqueForm::RegisterAddLink(create_link).basis(),
                        )
                    }
                    (DhtOpType::RegisterRemoveLink, Action::DeleteLink(delete_link)) => {
                        DhtOpLight::RegisterRemoveLink(
                            action_hash.clone(),
                            UniqueForm::RegisterRemoveLink(delete_link).basis(),
                        )
                    }
                    _ => return None,
                };
                Some(op_light)
            });
        ops.extend(op_lights);
    }
    Ok(ops)
}

/// Produce op types from a given [`Action`].
pub fn action_to_op_types(action: &Action) -> Vec<DhtOpType> {
    match action {
        Action::Dna(_)
        | Action::OpenChain(_)
        | Action::CloseChain(_)
        | Action::AgentValidationPkg(_)
        | Action::InitZomesComplete(_) => {
            vec![DhtOpType::StoreRecord, DhtOpType::RegisterAgentActivity]
        }
        Action::CreateLink(_) => vec![
            DhtOpType::StoreRecord,
            DhtOpType::RegisterAgentActivity,
            DhtOpType::RegisterAddLink,
        ],

        Action::DeleteLink(_) => vec![
            DhtOpType::StoreRecord,
            DhtOpType::RegisterAgentActivity,
            DhtOpType::RegisterRemoveLink,
        ],
        Action::Create(_) => vec![
            DhtOpType::StoreRecord,
            DhtOpType::RegisterAgentActivity,
            DhtOpType::StoreEntry,
        ],
        Action::Update(_) => vec![
            DhtOpType::StoreRecord,
            DhtOpType::RegisterAgentActivity,
            DhtOpType::StoreEntry,
            DhtOpType::RegisterUpdatedContent,
            DhtOpType::RegisterUpdatedRecord,
        ],
        Action::Delete(_) => vec![
            DhtOpType::StoreRecord,
            DhtOpType::RegisterAgentActivity,
            DhtOpType::RegisterDeletedBy,
            DhtOpType::RegisterDeletedEntryAction,
        ],
    }
}

// This has to be done manually because the macro
// implements both directions and that isn't possible with references
// TODO: Maybe add a one-way version to holochain_serialized_bytes?
impl<'a> TryFrom<&UniqueForm<'a>> for SerializedBytes {
    type Error = SerializedBytesError;
    fn try_from(u: &UniqueForm<'a>) -> Result<Self, Self::Error> {
        match holochain_serialized_bytes::encode(u) {
            Ok(v) => Ok(SerializedBytes::from(
                holochain_serialized_bytes::UnsafeBytes::from(v),
            )),
            Err(e) => Err(SerializedBytesError::Serialize(e.to_string())),
        }
    }
}

/// A DhtOp paired with its DhtOpHash
pub type DhtOpHashed = HoloHashed<DhtOp>;

impl HashableContent for DhtOp {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            (&self.as_unique_form())
                .try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

impl HashableContent for UniqueForm<'_> {
    type HashType = hash_type::DhtOp;

    fn hash_type(&self) -> Self::HashType {
        hash_type::DhtOp
    }

    fn hashable_content(&self) -> HashableContentBytes {
        HashableContentBytes::Content(
            self.try_into()
                .expect("Could not serialize HashableContent"),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, SerializedBytes)]
/// Condensed version of ops for sending across the wire.
pub enum WireOps {
    /// Response for get entry.
    Entry(WireEntryOps),
    /// Response for get record.
    Record(WireRecordOps),
}

impl WireOps {
    /// Render the wire ops to DhtOps.
    pub fn render(self) -> DhtOpResult<RenderedOps> {
        match self {
            WireOps::Entry(o) => o.render(),
            WireOps::Record(o) => o.render(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// The data rendered from a wire op to place in the database.
pub struct RenderedOp {
    /// The action to insert into the database.
    pub action: SignedActionHashed,
    /// The action to insert into the database.
    pub op_light: DhtOpLight,
    /// The hash of the [`DhtOp`]
    pub op_hash: DhtOpHash,
    /// The validation status of the action.
    pub validation_status: Option<ValidationStatus>,
}

impl RenderedOp {
    /// Try to create a new rendered op from wire data.
    /// This function computes all the hashes and
    /// reconstructs the full actions.
    pub fn new(
        action: Action,
        signature: Signature,
        validation_status: Option<ValidationStatus>,
        op_type: DhtOpType,
    ) -> DhtOpResult<Self> {
        let (action, op_hash) = UniqueForm::op_hash(op_type, action)?;
        let action_hashed = ActionHashed::from_content_sync(action);
        // TODO: Verify signature?
        let action = SignedActionHashed::with_presigned(action_hashed, signature);
        let op_light = DhtOpLight::from_type(op_type, action.as_hash().clone(), action.action())?;
        Ok(Self {
            action,
            op_light,
            op_hash,
            validation_status,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
/// The full data for insertion into the database.
/// The reason we don't use [`DhtOp`] is because we don't
/// want to clone the entry for every action.
pub struct RenderedOps {
    /// Entry for the ops if there is one.
    pub entry: Option<EntryHashed>,
    /// Op data to insert.
    pub ops: Vec<RenderedOp>,
}

/// Type for deriving ordering of DhtOps
/// Don't change the order of this enum unless
/// you mean to change the order we process ops
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum OpNumericalOrder {
    RegisterAgentActivity = 0,
    StoreEntry,
    StoreRecord,
    RegisterUpdatedContent,
    RegisterUpdatedRecord,
    RegisterDeletedBy,
    RegisterDeletedEntryAction,
    RegisterAddLink,
    RegisterRemoveLink,
}

/// This is used as an index for ordering ops in our database.
/// It gives the most likely ordering where dependencies will come
/// first.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct OpOrder {
    order: OpNumericalOrder,
    timestamp: holochain_zome_types::timestamp::Timestamp,
}

impl std::fmt::Display for OpOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{:019}",
            self.order as u8,
            // clamp unrealistic negative timestamps to 0
            i64::max(0, self.timestamp.as_micros())
        )
    }
}

impl OpOrder {
    /// Create a new ordering from a op type and timestamp.
    pub fn new(op_type: DhtOpType, timestamp: holochain_zome_types::timestamp::Timestamp) -> Self {
        let order = match op_type {
            DhtOpType::StoreRecord => OpNumericalOrder::StoreRecord,
            DhtOpType::StoreEntry => OpNumericalOrder::StoreEntry,
            DhtOpType::RegisterAgentActivity => OpNumericalOrder::RegisterAgentActivity,
            DhtOpType::RegisterUpdatedContent => OpNumericalOrder::RegisterUpdatedContent,
            DhtOpType::RegisterUpdatedRecord => OpNumericalOrder::RegisterUpdatedRecord,
            DhtOpType::RegisterDeletedBy => OpNumericalOrder::RegisterDeletedBy,
            DhtOpType::RegisterDeletedEntryAction => OpNumericalOrder::RegisterDeletedEntryAction,
            DhtOpType::RegisterAddLink => OpNumericalOrder::RegisterAddLink,
            DhtOpType::RegisterRemoveLink => OpNumericalOrder::RegisterRemoveLink,
        };
        Self { order, timestamp }
    }
}

impl holochain_sqlite::rusqlite::ToSql for OpOrder {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            self.to_string().into(),
        ))
    }
}
