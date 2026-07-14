//! [`OpHelper`] flattens an [`Op`] into a [`FlatOp`], for use in the
//! `validate` callback.

use crate::prelude::*;

/// Conversion from an [`Op`] to a [`FlatOp`], for use in the validate
/// callback.
pub trait OpHelper {
    /// Convert without consuming, cloning the required internal data.
    fn flattened<ET, LT>(&self) -> Result<crate::flat_op::FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>;
}

use crate::flat_op;

/// All possible variants that a [`RegisterAgentActivity`] with an
/// action that has an [`EntryType`] can produce.
#[derive(Debug)]
pub(crate) enum ActivityEntry<Unit> {
    App { entry_type: Option<Unit> },
    PrivateApp { entry_type: Option<Unit> },
    Agent(AgentPubKey),
    CapClaim,
    CapGrant,
}

impl OpHelper for Op {
    fn flattened<ET, LT>(&self) -> Result<flat_op::FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>,
    {
        match self {
            Op::StoreRecord(StoreRecord { record }) => {
                let a = record.action();
                let r = match &a.data {
                    ActionData::Dna(d) => flat_op::OpRecord::Dna {
                        dna_hash: d.dna_hash.clone(),
                        action: a.clone(),
                    },
                    ActionData::AgentValidationPkg(d) => flat_op::OpRecord::AgentValidationPkg {
                        membrane_proof: d.membrane_proof.clone(),
                        action: a.clone(),
                    },
                    ActionData::InitZomesComplete(_) => {
                        flat_op::OpRecord::InitZomesComplete { action: a.clone() }
                    }
                    ActionData::OpenChain(_) => flat_op::OpRecord::open_chain(a.clone()),
                    ActionData::CloseChain(_) => flat_op::OpRecord::close_chain(a.clone()),
                    ActionData::CreateLink(d) => {
                        let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                        flat_op::OpRecord::CreateLink {
                            base_address: d.base_address.clone(),
                            target_address: d.target_address.clone(),
                            tag: d.tag.clone(),
                            link_type,
                            action: a.clone(),
                        }
                    }
                    ActionData::DeleteLink(d) => flat_op::OpRecord::DeleteLink {
                        original_action_hash: d.link_add_address.clone(),
                        base_address: d.base_address.clone(),
                        action: a.clone(),
                    },
                    ActionData::Create(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op::OpRecord::CreateAgent {
                            agent: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => {
                            match get_app_entry_type_for_record_authority::<ET>(
                                entry_def,
                                record.entry.as_option(),
                            )? {
                                UnitEnumEither::Enum(app_entry) => flat_op::OpRecord::CreateEntry {
                                    app_entry,
                                    action: a.clone(),
                                },
                                UnitEnumEither::Unit(app_entry_type) => {
                                    flat_op::OpRecord::CreatePrivateEntry {
                                        app_entry_type,
                                        action: a.clone(),
                                    }
                                }
                            }
                        }
                        EntryType::CapClaim => {
                            flat_op::OpRecord::CreateCapClaim { action: a.clone() }
                        }
                        EntryType::CapGrant => {
                            flat_op::OpRecord::CreateCapGrant { action: a.clone() }
                        }
                    },
                    ActionData::Update(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op::OpRecord::UpdateAgent {
                            original_key: d.original_entry_address.clone().into(),
                            original_action_hash: d.original_action_address.clone(),
                            new_key: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => {
                            match get_app_entry_type_for_record_authority::<ET>(
                                entry_def,
                                record.entry.as_option(),
                            )? {
                                UnitEnumEither::Enum(app_entry) => flat_op::OpRecord::UpdateEntry {
                                    original_action_hash: d.original_action_address.clone(),
                                    original_entry_hash: d.original_entry_address.clone(),
                                    app_entry,
                                    action: a.clone(),
                                },
                                UnitEnumEither::Unit(app_entry_type) => {
                                    flat_op::OpRecord::UpdatePrivateEntry {
                                        original_action_hash: d.original_action_address.clone(),
                                        original_entry_hash: d.original_entry_address.clone(),
                                        app_entry_type,
                                        action: a.clone(),
                                    }
                                }
                            }
                        }
                        EntryType::CapClaim => flat_op::OpRecord::UpdateCapClaim {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            action: a.clone(),
                        },
                        EntryType::CapGrant => flat_op::OpRecord::UpdateCapGrant {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            action: a.clone(),
                        },
                    },
                    ActionData::Delete(d) => flat_op::OpRecord::DeleteEntry {
                        original_action_hash: d.deletes_address.clone(),
                        original_entry_hash: d.deletes_entry_address.clone(),
                        action: a.clone(),
                    },
                };
                Ok(flat_op::FlatOp::StoreRecord(r))
            }
            Op::StoreEntry(StoreEntry { action, entry }) => {
                let a = &action.hashed.content;
                let r = match &a.data {
                    ActionData::Create(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op::OpEntry::CreateAgent {
                            agent: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => flat_op::OpEntry::CreateEntry {
                            app_entry: get_app_entry_type_for_store_entry_authority(
                                entry_def, entry,
                            )?,
                            action: a.clone(),
                        },
                        EntryType::CapClaim => flat_op::OpEntry::CreateCapClaim {
                            entry: cap_claim_entry(entry)?,
                            action: a.clone(),
                        },
                        EntryType::CapGrant => flat_op::OpEntry::CreateCapGrant {
                            entry: cap_grant_entry(entry)?,
                            action: a.clone(),
                        },
                    },
                    ActionData::Update(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op::OpEntry::UpdateAgent {
                            original_key: d.original_entry_address.clone().into(),
                            original_action_hash: d.original_action_address.clone(),
                            new_key: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => flat_op::OpEntry::UpdateEntry {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            app_entry: get_app_entry_type_for_store_entry_authority(
                                entry_def, entry,
                            )?,
                            action: a.clone(),
                        },
                        EntryType::CapClaim => flat_op::OpEntry::UpdateCapClaim {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            entry: cap_claim_entry(entry)?,
                            action: a.clone(),
                        },
                        EntryType::CapGrant => flat_op::OpEntry::UpdateCapGrant {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            entry: cap_grant_entry(entry)?,
                            action: a.clone(),
                        },
                    },
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "StoreEntry op carried a non-entry-creation action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                Ok(flat_op::FlatOp::StoreEntry(r))
            }
            Op::RegisterUpdate(RegisterUpdate { update, new_entry }) => {
                let a = &update.hashed.content;
                let d = match &a.data {
                    ActionData::Update(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "RegisterUpdate op carried a non-Update action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let r = match &d.entry_type {
                    EntryType::AgentPubKey => flat_op::OpUpdate::Agent {
                        original_key: d.original_entry_address.clone().into(),
                        original_action_hash: d.original_action_address.clone(),
                        new_key: d.entry_hash.clone().into(),
                        action: a.clone(),
                    },
                    EntryType::App(entry_def) => {
                        match get_app_entry_type_for_record_authority::<ET>(
                            entry_def,
                            new_entry.as_ref(),
                        )? {
                            UnitEnumEither::Enum(new) => flat_op::OpUpdate::Entry {
                                app_entry: new,
                                action: a.clone(),
                            },
                            UnitEnumEither::Unit(new) => flat_op::OpUpdate::PrivateEntry {
                                original_action_hash: d.original_action_address.clone(),
                                app_entry_type: new,
                                action: a.clone(),
                            },
                        }
                    }
                    EntryType::CapClaim => flat_op::OpUpdate::CapClaim {
                        original_action_hash: d.original_action_address.clone(),
                        action: a.clone(),
                    },
                    EntryType::CapGrant => flat_op::OpUpdate::CapGrant {
                        original_action_hash: d.original_action_address.clone(),
                        action: a.clone(),
                    },
                };
                Ok(flat_op::FlatOp::RegisterUpdate(r))
            }
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
                let a = &action.hashed.content;
                let r = match &a.data {
                    ActionData::Dna(d) => flat_op::OpActivity::Dna {
                        dna_hash: d.dna_hash.clone(),
                        action: a.clone(),
                    },
                    ActionData::AgentValidationPkg(d) => flat_op::OpActivity::AgentValidationPkg {
                        membrane_proof: d.membrane_proof.clone(),
                        action: a.clone(),
                    },
                    ActionData::InitZomesComplete(_) => {
                        flat_op::OpActivity::InitZomesComplete { action: a.clone() }
                    }
                    ActionData::OpenChain(_) => flat_op::OpActivity::open_chain(a.clone()),
                    ActionData::CloseChain(_) => flat_op::OpActivity::close_chain(a.clone()),
                    ActionData::CreateLink(d) => {
                        let link_type = activity_link_type(d.zome_index, d.link_type)?;
                        flat_op::OpActivity::CreateLink {
                            base_address: d.base_address.clone(),
                            target_address: d.target_address.clone(),
                            tag: d.tag.clone(),
                            link_type,
                            action: a.clone(),
                        }
                    }
                    ActionData::DeleteLink(d) => flat_op::OpActivity::DeleteLink {
                        original_action_hash: d.link_add_address.clone(),
                        base_address: d.base_address.clone(),
                        action: a.clone(),
                    },
                    ActionData::Create(d) => {
                        match activity_entry::<ET>(&d.entry_type, &d.entry_hash)? {
                            ActivityEntry::App { entry_type, .. } => {
                                flat_op::OpActivity::CreateEntry {
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                flat_op::OpActivity::CreatePrivateEntry {
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::Agent(agent) => flat_op::OpActivity::CreateAgent {
                                agent,
                                action: a.clone(),
                            },
                            ActivityEntry::CapClaim => {
                                flat_op::OpActivity::CreateCapClaim { action: a.clone() }
                            }
                            ActivityEntry::CapGrant => {
                                flat_op::OpActivity::CreateCapGrant { action: a.clone() }
                            }
                        }
                    }
                    ActionData::Update(d) => {
                        match activity_entry::<ET>(&d.entry_type, &d.entry_hash)? {
                            ActivityEntry::App { entry_type, .. } => {
                                flat_op::OpActivity::UpdateEntry {
                                    original_action_hash: d.original_action_address.clone(),
                                    original_entry_hash: d.original_entry_address.clone(),
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                flat_op::OpActivity::UpdatePrivateEntry {
                                    original_action_hash: d.original_action_address.clone(),
                                    original_entry_hash: d.original_entry_address.clone(),
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::Agent(new_key) => flat_op::OpActivity::UpdateAgent {
                                original_action_hash: d.original_action_address.clone(),
                                original_key: d.original_entry_address.clone().into(),
                                new_key,
                                action: a.clone(),
                            },
                            ActivityEntry::CapClaim => flat_op::OpActivity::UpdateCapClaim {
                                original_action_hash: d.original_action_address.clone(),
                                original_entry_hash: d.original_entry_address.clone(),
                                action: a.clone(),
                            },
                            ActivityEntry::CapGrant => flat_op::OpActivity::UpdateCapGrant {
                                original_action_hash: d.original_action_address.clone(),
                                original_entry_hash: d.original_entry_address.clone(),
                                action: a.clone(),
                            },
                        }
                    }
                    ActionData::Delete(d) => flat_op::OpActivity::DeleteEntry {
                        original_action_hash: d.deletes_address.clone(),
                        original_entry_hash: d.deletes_entry_address.clone(),
                        action: a.clone(),
                    },
                };
                Ok(flat_op::FlatOp::RegisterAgentActivity(r))
            }
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                let a = &create_link.hashed.content;
                let d = match &a.data {
                    ActionData::CreateLink(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "RegisterCreateLink op carried a non-CreateLink action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                Ok(flat_op::FlatOp::RegisterLink(flat_op::OpLink::CreateLink {
                    base_address: d.base_address.clone(),
                    target_address: d.target_address.clone(),
                    tag: d.tag.clone(),
                    link_type,
                    action: a.clone(),
                }))
            }
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link,
                create_link,
            }) => {
                let delete_action = &delete_link.hashed.content;
                match &delete_action.data {
                    ActionData::DeleteLink(_) => {}
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "RegisterDeleteLink op carried a non-DeleteLink action: {:?}",
                            other.action_type()
                        ))))
                    }
                }
                let d = match &create_link.data {
                    ActionData::CreateLink(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "RegisterDeleteLink referenced a non-CreateLink original action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                Ok(flat_op::FlatOp::RegisterLink(flat_op::OpLink::DeleteLink {
                    original_action: create_link.clone(),
                    base_address: d.base_address.clone(),
                    target_address: d.target_address.clone(),
                    tag: d.tag.clone(),
                    link_type,
                    action: delete_action.clone(),
                }))
            }
            Op::RegisterDelete(RegisterDelete { delete }) => {
                let action = &delete.hashed.content;
                match &action.data {
                    ActionData::Delete(_) => {
                        Ok(flat_op::FlatOp::RegisterDelete(flat_op::OpDelete {
                            action: action.clone(),
                        }))
                    }
                    other => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "RegisterDelete op carried a non-Delete action: {:?}",
                        other.action_type()
                    )))),
                }
            }
        }
    }
}

/// Extract a `CapClaimEntry` from an entry, erroring if the entry is not one.
fn cap_claim_entry(entry: &Entry) -> Result<CapClaimEntry, WasmError> {
    match entry {
        Entry::CapClaim(e) => Ok(e.clone()),
        _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
            "Entry type does not match. CapClaim expected but got: {entry:?}"
        )))),
    }
}

/// Extract a `CapGrantEntry` from an entry, erroring if the entry is not one.
fn cap_grant_entry(entry: &Entry) -> Result<CapGrantEntry, WasmError> {
    match entry {
        Entry::CapGrant(e) => Ok(e.clone()),
        _ => Err(wasm_error!(WasmErrorInner::Guest(format!(
            "Entry type does not match. CapGrant expected but got: {entry:?}"
        )))),
    }
}

/// Produces the user-defined entry type enum. Even if the entry is private, this will succeed.
/// To be used only in the context of a StoreEntry authority.
pub(crate) fn get_app_entry_type_for_store_entry_authority<ET>(
    entry_def: &AppEntryDef,
    entry: &Entry,
) -> Result<ET, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
{
    let entry_type = <ET as EntryTypesHelper>::deserialize_from_type(
        entry_def.zome_index,
        entry_def.entry_index,
        entry,
    )?;
    match entry_type {
        Some(entry_type) => Ok(entry_type),
        None => Err(deny_other_zome()),
    }
}

/// Produces the user-defined entry type enum or the unit enum if entry is not present.
/// To be used only in the context of a StoreRecord or AgentActivity authority.
/// If the entry's availability does not match the defined visibility, an error will result.
pub(crate) fn get_app_entry_type_for_record_authority<ET>(
    entry_def: &AppEntryDef,
    entry: Option<&Entry>,
) -> Result<UnitEnumEither<ET>, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
{
    let AppEntryDef {
        zome_index,
        entry_index: entry_def_index,
        visibility,
        ..
    } = entry_def;
    match (entry, visibility) {
        (Some(entry), EntryVisibility::Public) => {
            get_app_entry_type_for_store_entry_authority(entry_def, entry).map(UnitEnumEither::Enum)
        }

        (None, EntryVisibility::Private) => {
            match get_unit_entry_type::<ET>(*zome_index, *entry_def_index)? {
                Some(unit) => Ok(UnitEnumEither::Unit(unit)),
                None => Err(deny_other_zome()),
            }
        }

        (Some(_), EntryVisibility::Private) => Err(wasm_error!(WasmErrorInner::Guest(format!(
            "Entry visibility is private but an entry was provided! entry_def: {entry_def:?}"
        )))),

        (None, EntryVisibility::Public) => Err(wasm_error!(WasmErrorInner::Guest(format!(
            "Entry visibility is public but no entry is available. entry_def: {entry_def:?}"
        )))),
    }
}

/// Maps [`RegisterAgentActivity`] ops to their
/// entries. The entry type will be [`None`] if
/// the zome id is not a dependency of this zome.
pub(crate) fn activity_entry<ET>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
) -> Result<ActivityEntry<<ET as UnitEnum>::Unit>, WasmError>
where
    ET: UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
{
    match entry_type {
        EntryType::App(AppEntryDef {
            zome_index,
            entry_index: entry_def_index,
            visibility,
        }) => {
            let unit = get_unit_entry_type::<ET>(*zome_index, *entry_def_index)?;
            match visibility {
                EntryVisibility::Public => Ok(ActivityEntry::App { entry_type: unit }),
                EntryVisibility::Private => Ok(ActivityEntry::PrivateApp { entry_type: unit }),
            }
        }
        EntryType::AgentPubKey => Ok(ActivityEntry::Agent(entry_hash.clone().into())),
        EntryType::CapClaim => Ok(ActivityEntry::CapClaim),
        EntryType::CapGrant => Ok(ActivityEntry::CapGrant),
    }
}

/// Get the app defined link type from a [`ZomeIndex`] and [`LinkType`].
/// If the [`ZomeIndex`] is not a dependency of this zome then return a host error.
pub(crate) fn in_scope_link_type<LT>(
    zome_index: ZomeIndex,
    link_type: LinkType,
) -> Result<LT, WasmError>
where
    LT: LinkTypesHelper,
    WasmError: From<<LT as LinkTypesHelper>::Error>,
{
    match <LT as LinkTypesHelper>::from_type(*zome_index, *link_type)? {
        Some(link_type) => Ok(link_type),
        None => Err(deny_other_zome()),
    }
}

/// Get the app defined link type from a [`ZomeIndex`] and [`LinkType`].
/// If the [`ZomeIndex`] is not a dependency of this zome then return a host error.
pub(crate) fn activity_link_type<LT>(
    zome_index: ZomeIndex,
    link_type: LinkType,
) -> Result<Option<LT>, WasmError>
where
    LT: LinkTypesHelper,
    WasmError: From<<LT as LinkTypesHelper>::Error>,
{
    Ok(<LT as LinkTypesHelper>::from_type(*zome_index, *link_type)?)
}

/// Produce the unit variant given a zome id and entry def index.
/// Returns [`None`] if the zome id is not a dependency of this zome.
/// Returns a [`WasmErrorInner::Guest`] error if the zome id is a
/// dependency but the [`EntryDefIndex`] is out of range.
fn get_unit_entry_type<ET>(
    zome_index: ZomeIndex,
    entry_def_index: EntryDefIndex,
) -> Result<Option<<ET as UnitEnum>::Unit>, WasmError>
where
    ET: UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
{
    let entries = zome_info()?.zome_types.entries;
    let unit = entries.find(
        <ET as UnitEnum>::unit_iter(),
        ScopedEntryDefIndex {
            zome_index,
            zome_type: entry_def_index,
        },
    );
    let unit = match unit {
        Some(unit) => Some(unit),
        None => {
            if entries.dependencies().any(|z| z == zome_index) {
                return Err(wasm_error!(WasmErrorInner::Guest(format!(
                    "Entry type: {entry_def_index:?} is out of range for this zome."
                ))));
            } else {
                None
            }
        }
    };
    Ok(unit)
}

/// Produce an error because this zome
/// should never be called with a zome id
/// that is not a dependency.
fn deny_other_zome() -> WasmError {
    wasm_error!(WasmErrorInner::Host(
        "Op called for zome it was not defined in. This is a Holochain bug".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as hdi;
    use crate::flat_op::{FlatOp, OpActivity, OpEntry, OpRecord};
    use crate::test_utils::set_zome_types;
    use crate::test_utils::short_hand::{e, public_app_entry_def};
    use holo_hash::{ActionHash, AgentPubKey, DnaHash, EntryHash};
    use holochain_integrity_types::record::{RecordEntry, SignedHashed};
    use holochain_integrity_types::signature::Signature;
    use holochain_integrity_types::{EntryType, LinkTag, LinkType, ZomeIndex};

    #[hdk_entry_helper]
    #[derive(Clone, PartialEq, Eq)]
    pub struct A;

    #[hdk_entry_types(skip_hdk_extern = true)]
    #[unit_enum(UnitEntryTypes)]
    #[derive(Clone, PartialEq, Eq)]
    pub enum EntryTypes {
        A(A),
    }

    #[hdk_link_types(skip_no_mangle = true)]
    pub enum LinkTypes {
        A,
    }

    fn signed_from_data(data: ActionData) -> SignedHashed<Action> {
        let action = Action {
            header: ActionHeader {
                author: AgentPubKey::from_raw_36(vec![1u8; 36]),
                timestamp: holochain_integrity_types::timestamp::Timestamp::from_micros(0),
                action_seq: 0,
                prev_action: None,
            },
            data,
        };
        let hash = ActionHash::from_raw_36(vec![9u8; 36]);
        SignedHashed::with_presigned(
            holo_hash::HoloHashed::with_pre_hashed(action, hash),
            Signature([0u8; 64]),
        )
    }

    fn create_app_data() -> ActionData {
        ActionData::Create(CreateData {
            entry_type: EntryType::App(public_app_entry_def(0, 0)),
            entry_hash: EntryHash::from_raw_36(vec![2u8; 36]),
        })
    }

    fn types() {
        set_zome_types(&[(0, 1)], &[(0, 1)]);
    }

    #[test]
    fn store_record_create_app_entry_flattens_to_create_entry() {
        types();
        let signed = signed_from_data(create_app_data());
        let record = Record::new(signed, RecordEntry::Present(e(A {})));
        let op = Op::StoreRecord(StoreRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::StoreRecord(OpRecord::CreateEntry {
                app_entry: EntryTypes::A(A {}),
                ..
            })
        ));
    }

    #[test]
    fn store_record_create_agent_flattens_to_create_agent() {
        types();
        let signed = signed_from_data(ActionData::Create(CreateData {
            entry_type: EntryType::AgentPubKey,
            entry_hash: EntryHash::from_raw_36(vec![3u8; 36]),
        }));
        let record = Record::new(signed, RecordEntry::NA);
        let op = Op::StoreRecord(StoreRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::StoreRecord(OpRecord::CreateAgent { .. })
        ));
    }

    #[test]
    fn store_record_dna_flattens_to_dna() {
        types();
        let signed = signed_from_data(ActionData::Dna(DnaData {
            dna_hash: DnaHash::from_raw_36(vec![4u8; 36]),
        }));
        let record = Record::new(signed, RecordEntry::NA);
        let op = Op::StoreRecord(StoreRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(flat, FlatOp::StoreRecord(OpRecord::Dna { .. })));
    }

    #[test]
    fn store_record_create_link_resolves_link_type() {
        types();
        let signed = signed_from_data(ActionData::CreateLink(CreateLinkData {
            base_address: EntryHash::from_raw_36(vec![5u8; 36]).into(),
            target_address: EntryHash::from_raw_36(vec![6u8; 36]).into(),
            zome_index: ZomeIndex(0),
            link_type: LinkType(0),
            tag: LinkTag(vec![]),
        }));
        let record = Record::new(signed, RecordEntry::NA);
        let op = Op::StoreRecord(StoreRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::StoreRecord(OpRecord::CreateLink {
                link_type: LinkTypes::A,
                ..
            })
        ));
    }

    #[test]
    fn store_entry_create_app_flattens_to_create_entry() {
        types();
        let signed = signed_from_data(create_app_data());
        let op = Op::StoreEntry(StoreEntry {
            action: signed,
            entry: e(A {}),
        });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::StoreEntry(OpEntry::CreateEntry {
                app_entry: EntryTypes::A(A {}),
                ..
            })
        ));
    }

    #[test]
    fn register_agent_activity_create_app_flattens_with_unit_type() {
        types();
        let signed = signed_from_data(create_app_data());
        let op = Op::RegisterAgentActivity(RegisterAgentActivity {
            action: signed,
            cached_entry: None,
        });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::RegisterAgentActivity(OpActivity::CreateEntry {
                app_entry_type: Some(UnitEntryTypes::A),
                ..
            })
        ));
    }

    #[test]
    fn register_delete_flattens_to_register_delete() {
        types();
        let signed = signed_from_data(ActionData::Delete(DeleteData {
            deletes_address: ActionHash::from_raw_36(vec![7u8; 36]),
            deletes_entry_address: EntryHash::from_raw_36(vec![8u8; 36]),
        }));
        let op = Op::RegisterDelete(RegisterDelete { delete: signed });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(flat, FlatOp::RegisterDelete(_)));
    }
}
