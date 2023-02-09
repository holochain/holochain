//! An alternative to [`Op`] using a flatter structure, and user-defined deserialized
//! entry included where appropriate

use crate::{
    Action, ActionRef, ActionType, AgentValidationPkg, AppEntryDef, CloseChain, Create, CreateLink,
    Delete, DeleteLink, Dna, EntryCreationAction, EntryType, InitZomesComplete, LinkTag,
    MembraneProof, OpenChain, UnitEnum, Update,
};
use holo_hash::{ActionHash, AgentPubKey, AnyLinkableHash, DnaHash, EntryHash, HashableContent};
use holochain_serialized_bytes::prelude::*;
use kitsune_p2p_timestamp::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A convenience type for validation [`Op`]s.
pub enum FlatOp<ET, LT>
where
    ET: UnitEnum,
{
    /// The [`Op::StoreRecord`] which is validated by the authority
    /// for the [`ActionHash`] of this record.
    ///
    /// This operation stores a [`Record`] on the DHT and is
    /// returned when the authority receives a request
    /// on the [`ActionHash`].
    StoreRecord(OpRecord<ET, LT>),
    /// The [`Op::StoreEntry`] which is validated by the authority
    /// for the [`EntryHash`] of this entry.
    ///
    /// This operation stores an [`Entry`] on the DHT and is
    /// returned when the authority receives a request
    /// on the [`EntryHash`].
    StoreEntry(OpEntry<ET>),
    /// The [`Op::RegisterAgentActivity`] which is validated by
    /// the authority for the [`AgentPubKey`] for the author of this [`Action`].
    ///
    /// This operation registers an [`Action`] to an agent's chain
    /// on the DHT and is returned when the authority receives a request
    /// on the [`AgentPubKey`] for chain data.
    ///
    /// Note that [`Op::RegisterAgentActivity`] is the only operation
    /// that is validated by all zomes regardless of entry or link types.
    RegisterAgentActivity(OpActivity<<ET as UnitEnum>::Unit, LT>),
    /// The [`Op::RegisterCreateLink`] which is validated by
    /// the authority for the [`AnyLinkableHash`] in the base address
    /// of this link.
    ///
    /// This operation register's a link to the base address
    /// on the DHT and is returned when the authority receives a request
    /// on the base [`AnyLinkableHash`] for links.
    RegisterCreateLink {
        /// The base address where this link is stored.
        base_address: AnyLinkableHash,
        /// The target address of this link.
        target_address: AnyLinkableHash,
        /// The link's tag data.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
        /// The [`CreateLink`] action that creates the link
        action: CreateLink,
    },
    /// The [`Op::RegisterDeleteLink`] which is validated by
    /// the authority for the [`AnyLinkableHash`] in the base address
    /// of the link that is being deleted.
    ///
    /// This operation registers a deletion of a link to the base address
    /// on the DHT and is returned when the authority receives a request
    /// on the base [`AnyLinkableHash`] for the link that is being deleted.
    RegisterDeleteLink {
        /// The original [`CreateLink`] [`Action`] that created the link.
        original_action: CreateLink,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The target address of the link being deleted.
        target_address: AnyLinkableHash,
        /// The deleted links tag data.
        tag: LinkTag,
        /// The app defined link type of the deleted link.
        link_type: LT,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// The [`Op::RegisterUpdate`] which is validated by
    /// the authority for the [`ActionHash`] of the original entry
    /// and the authority for the [`EntryHash`] of the original entry.
    ///
    /// This operation registers an update from the original entry on
    /// the DHT and is returned when the authority receives a request
    /// for the [`ActionHash`] of the original entry [`Action`] or the
    /// [`EntryHash`] of the original entry.
    RegisterUpdate(OpUpdate<ET>),
    /// The [`Op::RegisterDelete`] which is validated by
    /// the authority for the [`ActionHash`] of the deleted entry
    /// and the authority for the [`EntryHash`] of the deleted entry.
    ///
    /// This operation registers a deletion to the original entry on
    /// the DHT and is returned when the authority receives a request
    /// for the [`ActionHash`] of the deleted entry [`Action`] or the
    /// [`EntryHash`] of the deleted entry.
    RegisterDelete(OpDelete<ET>),
}

#[deprecated = "use the name FlatOp instead"]
/// Alias for `FlatOp` for backward compatibility
pub type OpType<ET, LT> = FlatOp<ET, LT>;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreRecord`] operation.
pub enum OpRecord<ET, LT>
where
    ET: UnitEnum,
{
    /// This operation stores the [`Record`] for an
    /// app defined entry type.
    CreateEntry {
        /// The app defined entry type with the deserialized
        /// [`Entry`] data.
        app_entry: ET,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`] for an
    /// app defined private entry type.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type.
        /// Note it is not possible to deserialize the full
        /// entry type here because we don't have the [`Entry`] data.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`] for an
    /// [`AgentPubKey`] that has been created.
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation stores the [`Record`] for a
    /// Capability Claim that has been created.
    CreateCapClaim {
        /// The [`Create`] action that creates the [`crate::CapClaim`]
        action: Create,
    },
    /// This operation stores the [`Record`] for a
    /// Capability Grant that has been created.
    CreateCapGrant {
        /// The [`Create`] action that creates the [`crate::CapGrant`]
        action: Create,
    },
    /// This operation stores the [`Record`] for an
    /// updated app defined entry type.
    UpdateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data from the new entry.
        /// Note the new entry type is always the same as the
        /// original entry type however the data may have changed.
        app_entry: ET,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated app defined private entry type.
    UpdatePrivateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// Note the new entry type is always the same as the
        /// original entry type however the data may have changed.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The hash of the [`Action`] that created the original key
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated Capability Claim.
    UpdateCapClaim {
        /// The hash of the [`Action`] that created the original [`crate::CapClaim`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapClaim`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapClaim`]
        action: Update,
    },
    /// This operation stores the [`Record`] for an
    /// updated Capability Grant.
    UpdateCapGrant {
        /// The hash of the [`Action`] that created the original [`crate::CapGrant`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapGrant`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapGrant`]
        action: Update,
    },
    /// This operation stores the [`Record`] for a
    /// deleted app defined entry type.
    DeleteEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The [`Delete`] action that creates the entry
        action: Delete,
    },
    /// This operation stores the [`Record`] for a
    /// new link.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        link_type: LT,
        /// The [`CreateLink`] action that creates this link
        action: CreateLink,
    },
    /// This operation stores the [`Record`] for a
    /// deleted link and contains the original link's
    /// [`Action`] hash.
    DeleteLink {
        /// The deleted links [`CreateLink`] [`Action`].
        original_action_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::Dna`].
    Dna {
        /// The hash of the DNA
        dna_hash: DnaHash,
        /// The [`Dna`] action
        action: Dna,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::OpenChain`] and contains the previous
    /// chains's [`DnaHash`].
    OpenChain {
        /// Hash of the prevous DNA that we are migrating from
        previous_dna_hash: DnaHash,
        /// The [`OpenChain`] action
        action: OpenChain,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::CloseChain`] and contains the new
    /// chains's [`DnaHash`].
    CloseChain {
        /// Hash of the new DNA that we are migrating to
        new_dna_hash: DnaHash,
        /// The [`CloseChain`] action
        action: CloseChain,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::AgentValidationPkg`] and contains
    /// the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA
        membrane_proof: Option<MembraneProof>,
        /// The [`AgentValidationPkg`] action
        action: AgentValidationPkg,
    },
    /// This operation stores the [`Record`] for an
    /// [`Action::InitZomesComplete`].
    InitZomesComplete {
        /// The [`InitZomesComplete`] action
        action: InitZomesComplete,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterAgentActivity`] operation.
pub enum OpActivity<UnitType, LT> {
    /// This operation registers the [`Action`] for an
    /// app defined entry type to the author's chain.
    CreateEntry {
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation registers the [`Action`] for an
    /// app defined private entry type to the author's chain.
    CreatePrivateEntry {
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation registers the [`Action`] for an
    /// [`AgentPubKey`] to the author's chain.
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates the entry
        action: Create,
    },
    /// This operation registers the [`Action`] for a
    /// Capability Claim to the author's chain.
    CreateCapClaim {
        /// The [`Create`] action that creates the [`crate::CapClaim`]
        action: Create,
    },
    /// This operation registers the [`Action`] for a
    /// Capability Grant to the author's chain.
    CreateCapGrant {
        /// The [`Create`] action that creates the [`crate::CapGrant`]
        action: Create,
    },
    /// This operation registers the [`Action`] for an
    /// updated app defined entry type to the author's chain.
    UpdateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated app defined private entry type to the author's chain.
    UpdatePrivateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The unit version of the app defined entry type.
        /// If this is [`None`] then the entry type is defined
        /// in a different zome.
        app_entry_type: Option<UnitType>,
        /// The [`Update`] action that updates the entry
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated [`AgentPubKey`] to the author's chain.
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the agent's key
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated Capability Claim to the author's chain.
    UpdateCapClaim {
        /// The hash of the [`Action`] that created the original [`crate::CapClaim`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapClaim`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapClaim`]
        action: Update,
    },
    /// This operation registers the [`Action`] for an
    /// updated Capability Grant to the author's chain.
    UpdateCapGrant {
        /// The hash of the [`Action`] that created the original [`crate::CapGrant`]
        original_action_hash: ActionHash,
        /// The hash of the original [`crate::CapGrant`]
        original_entry_hash: EntryHash,
        /// The [`Update`] action that updates the [`crate::CapGrant`]
        action: Update,
    },
    /// This operation registers the [`Action`] for a
    /// deleted app defined entry type to the author's chain.
    DeleteEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The action that deletes the original entry
        action: Delete,
    },
    /// This operation registers the [`Action`] for a
    /// new link to the author's chain.
    CreateLink {
        /// The base address of the link.
        base_address: AnyLinkableHash,
        /// The target address of the link.
        target_address: AnyLinkableHash,
        /// The link's tag.
        tag: LinkTag,
        /// The app defined link type of this link.
        /// If this is [`None`] then the link type is defined
        /// in a different zome.
        link_type: Option<LT>,
        /// The action that creates this link
        action: CreateLink,
    },
    /// This operation registers the [`Action`] for a
    /// deleted link to the author's chain and contains
    /// the original link's [`Action`] hash.
    DeleteLink {
        /// The deleted links [`CreateLink`] [`Action`].
        original_action_hash: ActionHash,
        /// The base address where this link is stored.
        /// This is the base address of the link that is being deleted.
        base_address: AnyLinkableHash,
        /// The [`DeleteLink`] action that deletes the link
        action: DeleteLink,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::Dna`] to the author's chain.
    Dna {
        /// The hash of the DNA
        dna_hash: DnaHash,
        /// The [`Dna`] action
        action: Dna,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::OpenChain`] to the author's chain
    /// and contains the previous chains's [`DnaHash`].
    OpenChain {
        /// Hash of the prevous DNA that we are migrating from
        previous_dna_hash: DnaHash,
        /// The [`OpenChain`] action
        action: OpenChain,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::CloseChain`] to the author's chain
    /// and contains the new chains's [`DnaHash`].
    CloseChain {
        /// Hash of the new DNA that we are migrating to
        new_dna_hash: DnaHash,
        /// The [`CloseChain`] action
        action: CloseChain,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::AgentValidationPkg`] to the author's chain
    /// and contains the membrane proof if there is one.
    AgentValidationPkg {
        /// The membrane proof proving that the agent is allowed to participate in this DNA
        membrane_proof: Option<MembraneProof>,
        /// The [`AgentValidationPkg`] action
        action: AgentValidationPkg,
    },
    /// This operation registers the [`Action`] for an
    /// [`Action::InitZomesComplete`] to the author's chain.
    InitZomesComplete {
        /// The [`InitZomesComplete`] action
        action: InitZomesComplete,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::StoreEntry`] operation.
pub enum OpEntry<ET>
where
    ET: UnitEnum,
{
    /// This operation stores the [`Entry`] for an
    /// app defined entry type.
    CreateEntry {
        /// The app defined entry with the deserialized
        /// [`Entry`] data.
        app_entry: ET,
        /// The [`Create`] action that creates this entry
        action: Create,
    },
    /// This operation stores the [`Entry`] for an
    /// [`AgentPubKey`].
    CreateAgent {
        /// The agent that was created
        agent: AgentPubKey,
        /// The [`Create`] action that creates this agent's key
        action: Create,
    },
    /// This operation stores the [`Entry`] for the
    /// newly created entry in an update.
    UpdateEntry {
        /// The hash of the [`Action`] that created the original entry
        original_action_hash: ActionHash,
        /// The hash of the original entry
        original_entry_hash: EntryHash,
        /// The app defined entry with the deserialized
        /// [`Entry`] data of the new entry.
        app_entry: ET,
        /// The [`Update`] action that updates this entry
        action: Update,
    },
    /// This operation stores the [`Entry`] for an
    /// updated [`AgentPubKey`].
    UpdateAgent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original keys [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates this entry
        action: Update,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterUpdate`] operation.
pub enum OpUpdate<ET>
where
    ET: UnitEnum,
{
    /// This operation registers an update from
    /// the original [`Entry`].
    Entry {
        /// The original [`Create`] or [`Update`] [`Action`].
        original_action: EntryCreationAction,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the original entry.
        original_app_entry: ET,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data of the new entry.
        app_entry: ET,
        /// The action that updates this entry
        action: Update,
    },
    /// This operation registers an update from
    /// the original private [`Entry`].
    PrivateEntry {
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The unit version of the app defined entry type
        /// for the original entry.
        original_app_entry_type: <ET as UnitEnum>::Unit,
        /// The unit version of the app defined entry type
        /// for the new entry.
        app_entry_type: <ET as UnitEnum>::Unit,
        /// The action that updates this entry
        action: Update,
    },
    /// This operation registers an update from
    /// the original [`AgentPubKey`].
    Agent {
        /// The new [`AgentPubKey`].
        new_key: AgentPubKey,
        /// The original [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the agent's key
        action: Update,
    },
    /// This operation registers an update from
    /// a Capability Claim.
    CapClaim {
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the [`crate::CapClaim`]
        action: Update,
    },
    /// This operation registers an update from
    /// a Capability Grant.
    CapGrant {
        /// The hash of the original original [`Action`].
        original_action_hash: ActionHash,
        /// The [`Update`] action that updates the [`crate::CapGrant`]
        action: Update,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Data specific to the [`Op::RegisterDelete`] operation.
pub enum OpDelete<ET>
where
    ET: UnitEnum,
{
    /// This operation registers a deletion to the
    /// original [`Entry`].
    Entry {
        /// The entries original [`Create`] or [`Update`] [`Action`].
        original_action: EntryCreationAction,
        /// The app defined entry type with the deserialized
        /// [`Entry`] data from the deleted entry.
        original_app_entry: ET,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to the
    /// original private [`Entry`].
    PrivateEntry {
        /// The entries original [`EntryCreationAction`].
        original_action: EntryCreationAction,
        /// The unit version of the app defined entry type
        /// of the deleted entry.
        original_app_entry_type: <ET as UnitEnum>::Unit,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to an
    /// [`AgentPubKey`].
    Agent {
        /// The deleted [`AgentPubKey`].
        original_key: AgentPubKey,
        /// The hash of the deleted keys [`Action`].
        original_action: EntryCreationAction,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to a
    /// Capability Claim.
    CapClaim {
        /// The deleted Capability Claim's [`Action`].
        original_action: EntryCreationAction,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
    /// This operation registers a deletion to a
    /// Capability Grant.
    CapGrant {
        /// The deleted Capability Claim's [`Action`].
        original_action: EntryCreationAction,
        /// The [`Delete`] action that deletes this entry
        action: Delete,
    },
}
