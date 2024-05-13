//! Data structures representing the operations that can be performed within a Holochain DHT.
//!
//! See the [item-level documentation for `DhtOp`][DhtOp] for more details.
//!
//! [DhtOp]: enum.DhtOp.html

use std::str::FromStr;

use crate::action::NewEntryAction;
use crate::prelude::*;
use crate::record::RecordGroup;
use holo_hash::*;
use holochain_sqlite::rusqlite::types::FromSql;
use holochain_sqlite::rusqlite::ToSql;
use holochain_zome_types::action;
use holochain_zome_types::prelude::*;
use kitsune_p2p_dht::region::RegionData;
use kitsune_p2p_dht::Loc;
use serde::Deserialize;
use serde::Serialize;

mod error;
pub use error::*;

#[cfg(test)]
mod tests;

/// A unit of DHT gossip. Used to notify an authority of new (meta)data to hold
/// as well as changes to the status of already held data.
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    SerializedBytes,
    Eq,
    PartialEq,
    Hash,
    derive_more::Display,
    derive_more::From,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum DhtOp {
    /// An op representing storage of some record information.
    ChainOp(ChainOp),
    /// TODO, new type of op
    WarrantOp(Warrant),
}

/// A unit of DHT gossip concerning source chain data.
#[derive(
    Clone, Debug, Serialize, Deserialize, SerializedBytes, Eq, PartialEq, Hash, derive_more::Display,
)]
#[cfg_attr(
    feature = "fuzzing",
    derive(arbitrary::Arbitrary, proptest_derive::Arbitrary)
)]
pub enum ChainOp {
    #[display(fmt = "StoreRecord")]
    /// Used to notify the authority for an action that it has been created.
    ///
    /// Conceptually, authorities receiving this `ChainOp` do three things:
    ///
    /// - Ensure that the record passes validation.
    /// - Store the action into their DHT shard.
    /// - Store the entry into their CAS.
    ///   - Note: they do not become responsible for keeping the set of
    ///     references from that entry up-to-date.
    StoreRecord(Signature, Action, RecordEntry),

    #[display(fmt = "StoreEntry")]
    /// Used to notify the authority for an entry that it has been created
    /// anew. (The same entry can be created more than once.)
    ///
    /// Conceptually, authorities receiving this `ChainOp` do four things:
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
    StoreEntry(Signature, NewEntryAction, Entry),

    #[display(fmt = "RegisterAgentActivity")]
    /// Used to notify the authority for an agent's public key that that agent
    /// has committed a new action.
    ///
    /// Conceptually, authorities receiving this `ChainOp` do three things:
    ///
    /// - Ensure that *the action alone* passes surface-level validation.
    /// - Store the action into their DHT shard.
    //   - FIXME: @artbrock, do they?
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
    RegisterUpdatedContent(Signature, action::Update, RecordEntry),

    #[display(fmt = "RegisterUpdatedRecord")]
    /// Op for updating a record.
    /// This is sent to the record authority.
    RegisterUpdatedRecord(Signature, action::Update, RecordEntry),

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

#[deprecated = "DhtOpLight is renamed to DhtOpLite"]
/// Old alias for DhtOpLite
pub type DhtOpLight = DhtOpLite;

/// A type for storing in databases that doesn't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    derive_more::Display,
    derive_more::From,
)]
pub enum DhtOpLite {
    Chain(ChainOpLite),
}

/// A type for storing in databases that doesn't need the actual
/// data. Everything is a hash of the type except the signatures.
#[allow(missing_docs)]
#[derive(Clone, Debug, Serialize, Deserialize, derive_more::Display)]
pub enum ChainOpLite {
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

impl PartialEq for ChainOpLite {
    fn eq(&self, other: &Self) -> bool {
        // The ops are the same if they are the same type on the same action hash.
        // We can't derive eq because `Option<EntryHash>` doesn't make the op different.
        // We can ignore the basis because the basis is derived from the action and op type.
        self.get_type() == other.get_type() && self.action_hash() == other.action_hash()
    }
}

impl Eq for ChainOpLite {}

impl std::hash::Hash for ChainOpLite {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_type().hash(state);
        self.action_hash().hash(state);
    }
}

/// This enum is used to encode just the enum variant of ChainOp
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
pub enum ChainOpType {
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

/// Unit enum type corresponding to the different types of DhtOp
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq, Hash, derive_more::From)]
pub enum DhtOpType {
    Chain(ChainOpType),
}

impl ToSql for DhtOpType {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        match self {
            DhtOpType::Chain(op) => op.to_sql(),
        }
    }
}

impl FromSql for DhtOpType {
    fn column_result(
        value: holochain_sqlite::rusqlite::types::ValueRef<'_>,
    ) -> holochain_sqlite::rusqlite::types::FromSqlResult<Self> {
        String::column_result(value)
            .and_then(|string| {
                ChainOpType::from_str(&string)
                    .map_err(|_| holochain_sqlite::rusqlite::types::FromSqlError::InvalidType)
            })
            .map(Into::into)
    }
}

/// A sys validation dependency
pub type SysValDep = Option<ActionHash>;

impl ChainOpType {
    /// Calculate the op's sys validation dependency action hash
    pub fn sys_validation_dependency(&self, action: &Action) -> SysValDep {
        match self {
            ChainOpType::StoreRecord | ChainOpType::StoreEntry => None,
            ChainOpType::RegisterAgentActivity => action
                .prev_action()
                .map(|p| Some(p.clone()))
                .unwrap_or_else(|| None),
            ChainOpType::RegisterUpdatedContent | ChainOpType::RegisterUpdatedRecord => {
                match action {
                    Action::Update(update) => Some(update.original_action_address.clone()),
                    _ => None,
                }
            }
            ChainOpType::RegisterDeletedBy | ChainOpType::RegisterDeletedEntryAction => {
                match action {
                    Action::Delete(delete) => Some(delete.deletes_address.clone()),
                    _ => None,
                }
            }
            ChainOpType::RegisterAddLink => None,
            ChainOpType::RegisterRemoveLink => match action {
                Action::DeleteLink(delete_link) => Some(delete_link.link_add_address.clone()),
                _ => None,
            },
        }
    }
}

impl ToSql for ChainOpType {
    fn to_sql(
        &self,
    ) -> holochain_sqlite::rusqlite::Result<holochain_sqlite::rusqlite::types::ToSqlOutput> {
        Ok(holochain_sqlite::rusqlite::types::ToSqlOutput::Owned(
            format!("{}", self).into(),
        ))
    }
}

impl FromSql for ChainOpType {
    fn column_result(
        value: holochain_sqlite::rusqlite::types::ValueRef<'_>,
    ) -> holochain_sqlite::rusqlite::types::FromSqlResult<Self> {
        String::column_result(value).and_then(|string| {
            ChainOpType::from_str(&string)
                .map_err(|_| holochain_sqlite::rusqlite::types::FromSqlError::InvalidType)
        })
    }
}

impl DhtOp {
    fn as_unique_form(&self) -> UniqueForm<'_> {
        match self {
            Self::ChainOp(op) => op.as_unique_form(),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// If this is a chain op, return that
    pub fn as_chain_op(&self) -> Option<&ChainOp> {
        match self {
            Self::ChainOp(op) => Some(op),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> DhtOpType {
        match self {
            Self::ChainOp(op) => DhtOpType::Chain(op.get_type()),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// Returns the basis hash which determines which agents will receive this DhtOp
    pub fn dht_basis(&self) -> OpBasis {
        self.as_unique_form().basis()
    }

    /// Get the signature for this op
    pub fn signature(&self) -> &Signature {
        match self {
            Self::ChainOp(op) => op.signature(),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    fn to_order(&self) -> OpOrder {
        match self {
            Self::ChainOp(op) => OpOrder::new(op.get_type(), op.timestamp()),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// Access to the Timestamp
    pub fn author(&self) -> AgentPubKey {
        match self {
            Self::ChainOp(op) => op.action().author().clone(),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// Access to the Timestamp
    pub fn timestamp(&self) -> Timestamp {
        match self {
            Self::ChainOp(op) => op.timestamp(),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// Convert a [DhtOp] to a [DhtOpLite] and basis
    pub fn to_lite(&self) -> DhtOpLite {
        match self {
            Self::ChainOp(op) => DhtOpLite::Chain(op.to_lite()),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
        }
    }

    /// Calculate the op's sys validation dependency action hash
    pub fn sys_validation_dependency(&self) -> SysValDep {
        match self {
            Self::ChainOp(op) => op.get_type().sys_validation_dependency(&op.action()),
            Self::WarrantOp(_op) => unreachable!("todo: warrants"),
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

impl ChainOp {
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

    /// Get the signature for this op
    pub fn signature(&self) -> &Signature {
        match self {
            Self::StoreRecord(s, _, _)
            | Self::StoreEntry(s, _, _)
            | Self::RegisterAgentActivity(s, _)
            | Self::RegisterUpdatedContent(s, _, _)
            | Self::RegisterUpdatedRecord(s, _, _)
            | Self::RegisterDeletedBy(s, _)
            | Self::RegisterDeletedEntryAction(s, _)
            | Self::RegisterAddLink(s, _)
            | Self::RegisterRemoveLink(s, _) => s,
        }
    }

    /// Convert a [ChainOp] to a [ChainOpLite] and basis
    pub fn to_lite(&self) -> ChainOpLite {
        let basis = self.dht_basis();
        match self {
            Self::StoreRecord(_, a, _) => {
                let e = a.entry_data().map(|(e, _)| e.clone());
                let h = ActionHash::with_data_sync(a);
                ChainOpLite::StoreRecord(h, e, basis)
            }
            Self::StoreEntry(_, a, _) => {
                let e = a.entry().clone();
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::StoreEntry(h, e, basis)
            }
            Self::RegisterAgentActivity(_, a) => {
                let h = ActionHash::with_data_sync(a);
                ChainOpLite::RegisterAgentActivity(h, basis)
            }
            Self::RegisterUpdatedContent(_, a, _) => {
                let e = a.entry_hash.clone();
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::RegisterUpdatedContent(h, e, basis)
            }
            Self::RegisterUpdatedRecord(_, a, _) => {
                let e = a.entry_hash.clone();
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::RegisterUpdatedRecord(h, e, basis)
            }
            Self::RegisterDeletedBy(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::RegisterDeletedBy(h, basis)
            }
            Self::RegisterDeletedEntryAction(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::RegisterDeletedEntryAction(h, basis)
            }
            Self::RegisterAddLink(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::RegisterAddLink(h, basis)
            }
            Self::RegisterRemoveLink(_, a) => {
                let h = ActionHash::with_data_sync(&Action::from(a.clone()));
                ChainOpLite::RegisterRemoveLink(h, basis)
            }
        }
    }

    /// Get the action from this op
    /// This requires cloning and converting the action
    /// as some ops don't hold the Action type
    pub fn action(&self) -> Action {
        match self {
            Self::StoreRecord(_, a, _) => a.clone(),
            Self::StoreEntry(_, a, _) => a.clone().into(),
            Self::RegisterAgentActivity(_, a) => a.clone(),
            Self::RegisterUpdatedContent(_, a, _) => a.clone().into(),
            Self::RegisterUpdatedRecord(_, a, _) => a.clone().into(),
            Self::RegisterDeletedBy(_, a) => a.clone().into(),
            Self::RegisterDeletedEntryAction(_, a) => a.clone().into(),
            Self::RegisterAddLink(_, a) => a.clone().into(),
            Self::RegisterRemoveLink(_, a) => a.clone().into(),
        }
    }

    /// Get the entry from this op, if one exists
    pub fn entry(&self) -> RecordEntryRef {
        match self {
            Self::StoreRecord(_, _, e) => e.as_ref(),
            Self::StoreEntry(_, _, e) => RecordEntry::Present(e),
            Self::RegisterUpdatedContent(_, _, e) => e.as_ref(),
            Self::RegisterUpdatedRecord(_, _, e) => e.as_ref(),
            Self::RegisterAgentActivity(_, a) => RecordEntry::new(a.entry_visibility(), None),
            Self::RegisterDeletedBy(_, _) => RecordEntry::NA,
            Self::RegisterDeletedEntryAction(_, _) => RecordEntry::NA,
            Self::RegisterAddLink(_, _) => RecordEntry::NA,
            Self::RegisterRemoveLink(_, _) => RecordEntry::NA,
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> ChainOpType {
        match self {
            Self::StoreRecord(_, _, _) => ChainOpType::StoreRecord,
            Self::StoreEntry(_, _, _) => ChainOpType::StoreEntry,
            Self::RegisterUpdatedContent(_, _, _) => ChainOpType::RegisterUpdatedContent,
            Self::RegisterUpdatedRecord(_, _, _) => ChainOpType::RegisterUpdatedRecord,
            Self::RegisterAgentActivity(_, _) => ChainOpType::RegisterAgentActivity,
            Self::RegisterDeletedBy(_, _) => ChainOpType::RegisterDeletedBy,
            Self::RegisterDeletedEntryAction(_, _) => ChainOpType::RegisterDeletedEntryAction,
            Self::RegisterAddLink(_, _) => ChainOpType::RegisterAddLink,
            Self::RegisterRemoveLink(_, _) => ChainOpType::RegisterRemoveLink,
        }
    }

    /// From a type, action and an entry (if there is one)
    pub fn from_type(
        op_type: ChainOpType,
        action: SignedAction,
        entry: Option<Entry>,
    ) -> DhtOpResult<Self> {
        let SignedAction(action, signature) = action;
        let entry = RecordEntry::new(action.entry_visibility(), entry);
        let r = match op_type {
            ChainOpType::StoreRecord => Self::StoreRecord(signature, action, entry),
            ChainOpType::StoreEntry => {
                let entry = entry
                    .into_option()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                let action = match action {
                    Action::Create(c) => NewEntryAction::Create(c),
                    Action::Update(c) => NewEntryAction::Update(c),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::StoreEntry(signature, action, entry)
            }
            ChainOpType::RegisterAgentActivity => Self::RegisterAgentActivity(signature, action),
            ChainOpType::RegisterUpdatedContent => {
                Self::RegisterUpdatedContent(signature, action.try_into()?, entry)
            }
            ChainOpType::RegisterUpdatedRecord => {
                Self::RegisterUpdatedRecord(signature, action.try_into()?, entry)
            }
            ChainOpType::RegisterDeletedBy => {
                Self::RegisterDeletedBy(signature, action.try_into()?)
            }
            ChainOpType::RegisterDeletedEntryAction => {
                Self::RegisterDeletedEntryAction(signature, action.try_into()?)
            }
            ChainOpType::RegisterAddLink => Self::RegisterAddLink(signature, action.try_into()?),
            ChainOpType::RegisterRemoveLink => {
                Self::RegisterRemoveLink(signature, action.try_into()?)
            }
        };
        Ok(r)
    }

    /// Enzymatic countersigning session ops need special handling so that they
    /// arrive at the enzyme and not elsewhere. If this isn't an enzymatic
    /// countersigning session then the return will be None so can be used as
    /// a boolean for filtering with is_some().
    pub fn enzymatic_countersigning_enzyme(&self) -> Option<&AgentPubKey> {
        if let Some(Entry::CounterSign(session_data, _)) = self.entry().into_option() {
            if session_data.preflight_request().enzymatic {
                session_data
                    .preflight_request()
                    .signing_agents
                    .first()
                    .map(|(pubkey, _)| pubkey)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Access to the Timestamp
    pub fn timestamp(&self) -> Timestamp {
        match self {
            ChainOp::StoreRecord(_, a, _) => a.timestamp(),
            ChainOp::StoreEntry(_, a, _) => a.timestamp(),
            ChainOp::RegisterAgentActivity(_, a) => a.timestamp(),
            ChainOp::RegisterUpdatedContent(_, a, _) => a.timestamp,
            ChainOp::RegisterUpdatedRecord(_, a, _) => a.timestamp,
            ChainOp::RegisterDeletedBy(_, a) => a.timestamp,
            ChainOp::RegisterDeletedEntryAction(_, a) => a.timestamp,
            ChainOp::RegisterAddLink(_, a) => a.timestamp,
            ChainOp::RegisterRemoveLink(_, a) => a.timestamp,
        }
    }
}

impl DhtOpLite {
    /// Get the dht basis for where to send this op
    pub fn dht_basis(&self) -> &OpBasis {
        match self {
            Self::Chain(op) => op.dht_basis(),
        }
    }

    /// If this is a chain op, return it
    pub fn as_chain_op(&self) -> Option<&ChainOpLite> {
        match self {
            Self::Chain(op) => Some(op),
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> DhtOpType {
        match self {
            Self::Chain(op) => op.get_type().into(),
        }
    }

    /// Get the AnyDhtHash which would be used in a `must_get_*` context.
    ///
    /// For instance, `must_get_entry` will use an EntryHash, and requires a
    /// StoreEntry record to be integrated to succeed. All other must_gets take
    /// an ActionHash.
    pub fn fetch_dependency_hash(&self) -> AnyDhtHash {
        match self {
            Self::Chain(op) => match op {
                ChainOpLite::StoreEntry(_, entry_hash, _) => entry_hash.clone().into(),
                other => other.action_hash().clone().into(),
            },
        }
    }
}

impl ChainOpLite {
    /// Get the dht basis for where to send this op
    pub fn dht_basis(&self) -> &OpBasis {
        match self {
            ChainOpLite::StoreRecord(_, _, b)
            | ChainOpLite::StoreEntry(_, _, b)
            | ChainOpLite::RegisterAgentActivity(_, b)
            | ChainOpLite::RegisterUpdatedContent(_, _, b)
            | ChainOpLite::RegisterUpdatedRecord(_, _, b)
            | ChainOpLite::RegisterDeletedBy(_, b)
            | ChainOpLite::RegisterDeletedEntryAction(_, b)
            | ChainOpLite::RegisterAddLink(_, b)
            | ChainOpLite::RegisterRemoveLink(_, b) => b,
        }
    }

    /// Get the action hash from this op
    pub fn action_hash(&self) -> &ActionHash {
        match self {
            Self::StoreRecord(h, _, _)
            | Self::StoreEntry(h, _, _)
            | Self::RegisterAgentActivity(h, _)
            | Self::RegisterUpdatedContent(h, _, _)
            | Self::RegisterUpdatedRecord(h, _, _)
            | Self::RegisterDeletedBy(h, _)
            | Self::RegisterDeletedEntryAction(h, _)
            | Self::RegisterAddLink(h, _)
            | Self::RegisterRemoveLink(h, _) => h,
        }
    }

    /// Get the type as a unit enum, for Display purposes
    pub fn get_type(&self) -> ChainOpType {
        match self {
            Self::StoreRecord(_, _, _) => ChainOpType::StoreRecord,
            Self::StoreEntry(_, _, _) => ChainOpType::StoreEntry,
            Self::RegisterUpdatedContent(_, _, _) => ChainOpType::RegisterUpdatedContent,
            Self::RegisterUpdatedRecord(_, _, _) => ChainOpType::RegisterUpdatedRecord,
            Self::RegisterAgentActivity(_, _) => ChainOpType::RegisterAgentActivity,
            Self::RegisterDeletedBy(_, _) => ChainOpType::RegisterDeletedBy,
            Self::RegisterDeletedEntryAction(_, _) => ChainOpType::RegisterDeletedEntryAction,
            Self::RegisterAddLink(_, _) => ChainOpType::RegisterAddLink,
            Self::RegisterRemoveLink(_, _) => ChainOpType::RegisterRemoveLink,
        }
    }

    /// From a type with the hashes.
    pub fn from_type(
        op_type: ChainOpType,
        action_hash: ActionHash,
        action: &Action,
    ) -> DhtOpResult<Self> {
        let op = match op_type {
            ChainOpType::StoreRecord => {
                let entry_hash = action.entry_hash().cloned();
                Self::StoreRecord(action_hash.clone(), entry_hash, action_hash.into())
            }
            ChainOpType::StoreEntry => {
                let entry_hash = action
                    .entry_hash()
                    .cloned()
                    .ok_or_else(|| DhtOpError::ActionWithoutEntry(action.clone()))?;
                Self::StoreEntry(action_hash, entry_hash.clone(), entry_hash.into())
            }
            ChainOpType::RegisterAgentActivity => {
                Self::RegisterAgentActivity(action_hash, action.author().clone().into())
            }
            ChainOpType::RegisterUpdatedContent => {
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
            ChainOpType::RegisterUpdatedRecord => {
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
            ChainOpType::RegisterDeletedBy => {
                let basis = match action {
                    Action::Delete(delete) => delete.deletes_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterDeletedBy(action_hash, basis.into())
            }
            ChainOpType::RegisterDeletedEntryAction => {
                let basis = match action {
                    Action::Delete(delete) => delete.deletes_entry_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterDeletedEntryAction(action_hash, basis.into())
            }
            ChainOpType::RegisterAddLink => {
                let basis = match action {
                    Action::CreateLink(create_link) => create_link.base_address.clone(),
                    _ => return Err(DhtOpError::OpActionMismatch(op_type, action.action_type())),
                };
                Self::RegisterAddLink(action_hash, basis)
            }
            ChainOpType::RegisterRemoveLink => {
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
    pub fn op_hash(op_type: ChainOpType, action: Action) -> DhtOpResult<(Action, DhtOpHash)> {
        match op_type {
            ChainOpType::StoreRecord => {
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreRecord(&action));
                Ok((action, hash))
            }
            ChainOpType::StoreEntry => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::StoreEntry(&action));
                Ok((action.into(), hash))
            }
            ChainOpType::RegisterAgentActivity => {
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAgentActivity(&action));
                Ok((action, hash))
            }
            ChainOpType::RegisterUpdatedContent => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterUpdatedContent(&action));
                Ok((action.into(), hash))
            }
            ChainOpType::RegisterUpdatedRecord => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterUpdatedRecord(&action));
                Ok((action.into(), hash))
            }
            ChainOpType::RegisterDeletedBy => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterDeletedBy(&action));
                Ok((action.into(), hash))
            }
            ChainOpType::RegisterDeletedEntryAction => {
                let action = action.try_into()?;
                let hash =
                    DhtOpHash::with_data_sync(&UniqueForm::RegisterDeletedEntryAction(&action));
                Ok((action.into(), hash))
            }
            ChainOpType::RegisterAddLink => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterAddLink(&action));
                Ok((action.into(), hash))
            }
            ChainOpType::RegisterRemoveLink => {
                let action = action.try_into()?;
                let hash = DhtOpHash::with_data_sync(&UniqueForm::RegisterRemoveLink(&action));
                Ok((action.into(), hash))
            }
        }
    }
}

/// Produce all DhtOps for a Record
pub fn produce_ops_from_record(record: &Record) -> DhtOpResult<Vec<ChainOp>> {
    let op_lites = produce_op_lites_from_records(vec![record])?;
    let (shh, entry) = record.clone().into_inner();
    let SignedActionHashed {
        hashed: ActionHashed {
            content: action, ..
        },
        signature,
    } = shh;

    let mut ops = Vec::with_capacity(op_lites.len());

    for op_light in op_lites {
        let signature = signature.clone();
        let action = action.clone();
        let op = match op_light {
            ChainOpLite::StoreRecord(_, _, _) => {
                ChainOp::StoreRecord(signature, action, entry.clone())
            }
            ChainOpLite::StoreEntry(_, _, _) => {
                let new_entry_action = action.clone().try_into()?;
                let e = match entry.clone().into_option() {
                    Some(e) => e,
                    None => {
                        // Entry is private so continue
                        continue;
                    }
                };
                ChainOp::StoreEntry(signature, new_entry_action, e)
            }
            ChainOpLite::RegisterAgentActivity(_, _) => {
                ChainOp::RegisterAgentActivity(signature, action)
            }
            ChainOpLite::RegisterUpdatedContent(_, _, _) => {
                let entry_update = action.try_into()?;
                ChainOp::RegisterUpdatedContent(signature, entry_update, entry.clone())
            }
            ChainOpLite::RegisterUpdatedRecord(_, _, _) => {
                let entry_update = action.try_into()?;
                ChainOp::RegisterUpdatedRecord(signature, entry_update, entry.clone())
            }
            ChainOpLite::RegisterDeletedEntryAction(_, _) => {
                let record_delete = action.try_into()?;
                ChainOp::RegisterDeletedEntryAction(signature, record_delete)
            }
            ChainOpLite::RegisterDeletedBy(_, _) => {
                let record_delete = action.try_into()?;
                ChainOp::RegisterDeletedBy(signature, record_delete)
            }
            ChainOpLite::RegisterAddLink(_, _) => {
                let link_add = action.try_into()?;
                ChainOp::RegisterAddLink(signature, link_add)
            }
            ChainOpLite::RegisterRemoveLink(_, _) => {
                let link_remove = action.try_into()?;
                ChainOp::RegisterRemoveLink(signature, link_remove)
            }
        };
        ops.push(op);
    }
    Ok(ops)
}

/// Produce all the op lites for these records
pub fn produce_op_lites_from_records(actions: Vec<&Record>) -> DhtOpResult<Vec<ChainOpLite>> {
    let actions_and_hashes = actions.into_iter().map(|e| {
        (
            e.action_address(),
            e.action(),
            e.action().entry_data().map(|(h, _)| h.clone()),
        )
    });
    produce_op_lites_from_iter(actions_and_hashes)
}

/// Produce all the op lites from this record group
/// with a shared entry
pub fn produce_op_lites_from_record_group(
    records: &RecordGroup<'_>,
) -> DhtOpResult<Vec<ChainOpLite>> {
    let actions_and_hashes = records.actions_and_hashes();
    let maybe_entry_hash = Some(records.entry_hash());
    produce_op_lites_from_parts(actions_and_hashes, maybe_entry_hash)
}

/// Data minimal clone (no cloning entries) cheap &Record to DhtOpLite conversion
fn produce_op_lites_from_parts<'a>(
    actions_and_hashes: impl Iterator<Item = (&'a ActionHash, &'a Action)>,
    maybe_entry_hash: Option<&EntryHash>,
) -> DhtOpResult<Vec<ChainOpLite>> {
    let iter = actions_and_hashes.map(|(head, hash)| (head, hash, maybe_entry_hash.cloned()));
    produce_op_lites_from_iter(iter)
}

/// Produce op lites from iter of (action hash, action, maybe entry).
pub fn produce_op_lites_from_iter<'a>(
    iter: impl Iterator<Item = (&'a ActionHash, &'a Action, Option<EntryHash>)>,
) -> DhtOpResult<Vec<ChainOpLite>> {
    let mut ops = Vec::new();

    for (action_hash, action, maybe_entry_hash) in iter {
        let op_lites = action_to_op_types(action)
            .into_iter()
            .filter_map(|op_type| {
                let op_light = match (op_type, action) {
                    (ChainOpType::StoreRecord, _) => {
                        let store_record_basis = UniqueForm::StoreRecord(action).basis();
                        ChainOpLite::StoreRecord(
                            action_hash.clone(),
                            maybe_entry_hash.clone(),
                            store_record_basis,
                        )
                    }
                    (ChainOpType::RegisterAgentActivity, _) => {
                        let register_activity_basis =
                            UniqueForm::RegisterAgentActivity(action).basis();
                        ChainOpLite::RegisterAgentActivity(
                            action_hash.clone(),
                            register_activity_basis,
                        )
                    }
                    (ChainOpType::StoreEntry, Action::Create(create)) => ChainOpLite::StoreEntry(
                        action_hash.clone(),
                        maybe_entry_hash.clone()?,
                        UniqueForm::StoreEntry(&NewEntryAction::Create(create.clone())).basis(),
                    ),
                    (ChainOpType::StoreEntry, Action::Update(update)) => ChainOpLite::StoreEntry(
                        action_hash.clone(),
                        maybe_entry_hash.clone()?,
                        UniqueForm::StoreEntry(&NewEntryAction::Update(update.clone())).basis(),
                    ),
                    (ChainOpType::RegisterUpdatedContent, Action::Update(update)) => {
                        ChainOpLite::RegisterUpdatedContent(
                            action_hash.clone(),
                            maybe_entry_hash.clone()?,
                            UniqueForm::RegisterUpdatedContent(update).basis(),
                        )
                    }
                    (ChainOpType::RegisterUpdatedRecord, Action::Update(update)) => {
                        ChainOpLite::RegisterUpdatedRecord(
                            action_hash.clone(),
                            maybe_entry_hash.clone()?,
                            UniqueForm::RegisterUpdatedRecord(update).basis(),
                        )
                    }
                    (ChainOpType::RegisterDeletedBy, Action::Delete(delete)) => {
                        ChainOpLite::RegisterDeletedBy(
                            action_hash.clone(),
                            UniqueForm::RegisterDeletedBy(delete).basis(),
                        )
                    }
                    (ChainOpType::RegisterDeletedEntryAction, Action::Delete(delete)) => {
                        ChainOpLite::RegisterDeletedEntryAction(
                            action_hash.clone(),
                            UniqueForm::RegisterDeletedEntryAction(delete).basis(),
                        )
                    }
                    (ChainOpType::RegisterAddLink, Action::CreateLink(create_link)) => {
                        ChainOpLite::RegisterAddLink(
                            action_hash.clone(),
                            UniqueForm::RegisterAddLink(create_link).basis(),
                        )
                    }
                    (ChainOpType::RegisterRemoveLink, Action::DeleteLink(delete_link)) => {
                        ChainOpLite::RegisterRemoveLink(
                            action_hash.clone(),
                            UniqueForm::RegisterRemoveLink(delete_link).basis(),
                        )
                    }
                    _ => return None,
                };
                Some(op_light)
            });
        ops.extend(op_lites);
    }
    Ok(ops)
}

/// Produce op types from a given [`Action`].
pub fn action_to_op_types(action: &Action) -> Vec<ChainOpType> {
    match action {
        Action::Dna(_)
        | Action::OpenChain(_)
        | Action::CloseChain(_)
        | Action::AgentValidationPkg(_)
        | Action::InitZomesComplete(_) => {
            vec![ChainOpType::StoreRecord, ChainOpType::RegisterAgentActivity]
        }
        Action::CreateLink(_) => vec![
            ChainOpType::StoreRecord,
            ChainOpType::RegisterAgentActivity,
            ChainOpType::RegisterAddLink,
        ],

        Action::DeleteLink(_) => vec![
            ChainOpType::StoreRecord,
            ChainOpType::RegisterAgentActivity,
            ChainOpType::RegisterRemoveLink,
        ],
        Action::Create(_) => vec![
            ChainOpType::StoreRecord,
            ChainOpType::RegisterAgentActivity,
            ChainOpType::StoreEntry,
        ],
        Action::Update(_) => vec![
            ChainOpType::StoreRecord,
            ChainOpType::RegisterAgentActivity,
            ChainOpType::StoreEntry,
            ChainOpType::RegisterUpdatedContent,
            ChainOpType::RegisterUpdatedRecord,
        ],
        Action::Delete(_) => vec![
            ChainOpType::StoreRecord,
            ChainOpType::RegisterAgentActivity,
            ChainOpType::RegisterDeletedBy,
            ChainOpType::RegisterDeletedEntryAction,
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

/// A ChainOp paired with its ChainOpHash
pub type ChainOpHashed = HoloHashed<ChainOp>;

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

impl HashableContent for ChainOp {
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
    pub op_light: DhtOpLite,
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
        op_type: ChainOpType,
    ) -> DhtOpResult<Self> {
        let (action, op_hash) = UniqueForm::op_hash(op_type, action)?;
        let action_hashed = ActionHashed::from_content_sync(action);
        // TODO: Verify signature?
        let action = SignedActionHashed::with_presigned(action_hashed, signature);
        let op_light =
            ChainOpLite::from_type(op_type, action.as_hash().clone(), action.action())?.into();
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
    pub fn new(
        op_type: impl Into<DhtOpType>,
        timestamp: holochain_zome_types::timestamp::Timestamp,
    ) -> Self {
        let order = match op_type.into() {
            DhtOpType::Chain(ChainOpType::StoreRecord) => OpNumericalOrder::StoreRecord,
            DhtOpType::Chain(ChainOpType::StoreEntry) => OpNumericalOrder::StoreEntry,
            DhtOpType::Chain(ChainOpType::RegisterAgentActivity) => {
                OpNumericalOrder::RegisterAgentActivity
            }
            DhtOpType::Chain(ChainOpType::RegisterUpdatedContent) => {
                OpNumericalOrder::RegisterUpdatedContent
            }
            DhtOpType::Chain(ChainOpType::RegisterUpdatedRecord) => {
                OpNumericalOrder::RegisterUpdatedRecord
            }
            DhtOpType::Chain(ChainOpType::RegisterDeletedBy) => OpNumericalOrder::RegisterDeletedBy,
            DhtOpType::Chain(ChainOpType::RegisterDeletedEntryAction) => {
                OpNumericalOrder::RegisterDeletedEntryAction
            }
            DhtOpType::Chain(ChainOpType::RegisterAddLink) => OpNumericalOrder::RegisterAddLink,
            DhtOpType::Chain(ChainOpType::RegisterRemoveLink) => {
                OpNumericalOrder::RegisterRemoveLink
            }
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
