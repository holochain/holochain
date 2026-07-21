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

/// All possible variants that a [`AgentActivity`] with an
/// action that has an [`EntryType`] can produce.
#[derive(Debug)]
pub(crate) enum ActivityEntry<Unit> {
    App { entry_type: Option<Unit> },
    PrivateApp { entry_type: Option<Unit> },
    Agent,
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
            Op::CreateRecord(CreateRecord { record }) => {
                let a = record.action();
                let r = match &a.data {
                    ActionData::Dna(d) => flat_op::OpRecord::Dna {
                        dna_hash: d.dna_hash.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::AgentValidationPkg(d) => flat_op::OpRecord::AgentValidationPkg {
                        membrane_proof: d.membrane_proof.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::InitZomesComplete(d) => flat_op::OpRecord::InitZomesComplete {
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::OpenChain(d) => flat_op::OpRecord::OpenChain {
                        previous_target: d.prev_target.clone(),
                        close_hash: d.close_hash.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::CloseChain(d) => flat_op::OpRecord::CloseChain {
                        new_target: d.new_target.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::CreateLink(d) => {
                        let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                        flat_op::OpRecord::CreateLink {
                            link_type,
                            action: TypedAction {
                                header: a.header.clone(),
                                data: d.clone(),
                            },
                        }
                    }
                    ActionData::DeleteLink(d) => flat_op::OpRecord::DeleteLink {
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::Create(d) => {
                        let typed_action = TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        };
                        match &d.entry_type {
                            EntryType::AgentPubKey => flat_op::OpRecord::CreateAgent {
                                action: typed_action,
                            },
                            EntryType::App(entry_def) => {
                                match get_app_entry_type_for_record_authority::<ET>(
                                    entry_def,
                                    record.entry.as_option(),
                                )? {
                                    UnitEnumEither::Enum(app_entry) => {
                                        flat_op::OpRecord::CreateEntry {
                                            app_entry,
                                            action: typed_action,
                                        }
                                    }
                                    UnitEnumEither::Unit(app_entry_type) => {
                                        flat_op::OpRecord::CreatePrivateEntry {
                                            app_entry_type,
                                            action: typed_action,
                                        }
                                    }
                                }
                            }
                            EntryType::CapClaim => flat_op::OpRecord::CreateCapClaim {
                                action: typed_action,
                            },
                            EntryType::CapGrant => flat_op::OpRecord::CreateCapGrant {
                                action: typed_action,
                            },
                        }
                    }
                    ActionData::Update(d) => {
                        let typed_action = TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        };
                        match &d.entry_type {
                            EntryType::AgentPubKey => flat_op::OpRecord::UpdateAgent {
                                action: typed_action,
                            },
                            EntryType::App(entry_def) => {
                                match get_app_entry_type_for_record_authority::<ET>(
                                    entry_def,
                                    record.entry.as_option(),
                                )? {
                                    UnitEnumEither::Enum(app_entry) => {
                                        flat_op::OpRecord::UpdateEntry {
                                            app_entry,
                                            action: typed_action,
                                        }
                                    }
                                    UnitEnumEither::Unit(app_entry_type) => {
                                        flat_op::OpRecord::UpdatePrivateEntry {
                                            app_entry_type,
                                            action: typed_action,
                                        }
                                    }
                                }
                            }
                            EntryType::CapClaim => flat_op::OpRecord::UpdateCapClaim {
                                action: typed_action,
                            },
                            EntryType::CapGrant => flat_op::OpRecord::UpdateCapGrant {
                                action: typed_action,
                            },
                        }
                    }
                    ActionData::Delete(d) => flat_op::OpRecord::DeleteEntry {
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                };
                Ok(flat_op::FlatOp::CreateRecord(r))
            }
            Op::CreateEntry(CreateEntry { action, entry }) => {
                let a = &action.hashed.content;
                let r = match &a.data {
                    ActionData::Create(d) => {
                        let typed_action = TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        };
                        match &d.entry_type {
                            EntryType::AgentPubKey => flat_op::OpEntry::CreateAgent {
                                action: typed_action,
                            },
                            EntryType::App(entry_def) => flat_op::OpEntry::CreateEntry {
                                app_entry: get_app_entry_type_for_store_entry_authority(
                                    entry_def, entry,
                                )?,
                                action: typed_action,
                            },
                            EntryType::CapClaim => flat_op::OpEntry::CreateCapClaim {
                                entry: cap_claim_entry(entry)?,
                                action: typed_action,
                            },
                            EntryType::CapGrant => flat_op::OpEntry::CreateCapGrant {
                                entry: cap_grant_entry(entry)?,
                                action: typed_action,
                            },
                        }
                    }
                    ActionData::Update(d) => {
                        let typed_action = TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        };
                        match &d.entry_type {
                            EntryType::AgentPubKey => flat_op::OpEntry::UpdateAgent {
                                action: typed_action,
                            },
                            EntryType::App(entry_def) => flat_op::OpEntry::UpdateEntry {
                                app_entry: get_app_entry_type_for_store_entry_authority(
                                    entry_def, entry,
                                )?,
                                action: typed_action,
                            },
                            EntryType::CapClaim => flat_op::OpEntry::UpdateCapClaim {
                                entry: cap_claim_entry(entry)?,
                                action: typed_action,
                            },
                            EntryType::CapGrant => flat_op::OpEntry::UpdateCapGrant {
                                entry: cap_grant_entry(entry)?,
                                action: typed_action,
                            },
                        }
                    }
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "CreateEntry op carried a non-entry-creation action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                Ok(flat_op::FlatOp::CreateEntry(r))
            }
            Op::Update(Update { update, new_entry }) => {
                let a = &update.hashed.content;
                let d = match &a.data {
                    ActionData::Update(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "Update op carried a non-Update action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let typed_action = TypedAction {
                    header: a.header.clone(),
                    data: d.clone(),
                };
                let r = match &d.entry_type {
                    EntryType::AgentPubKey => flat_op::OpUpdate::Agent {
                        original_key: d.original_entry_address.clone().into(),
                        new_key: d.entry_hash.clone().into(),
                        action: typed_action,
                    },
                    EntryType::App(entry_def) => {
                        match get_app_entry_type_for_record_authority::<ET>(
                            entry_def,
                            new_entry.as_ref(),
                        )? {
                            UnitEnumEither::Enum(new) => flat_op::OpUpdate::Entry {
                                app_entry: new,
                                action: typed_action,
                            },
                            UnitEnumEither::Unit(new) => flat_op::OpUpdate::PrivateEntry {
                                app_entry_type: new,
                                action: typed_action,
                            },
                        }
                    }
                    EntryType::CapClaim => flat_op::OpUpdate::CapClaim {
                        action: typed_action,
                    },
                    EntryType::CapGrant => flat_op::OpUpdate::CapGrant {
                        action: typed_action,
                    },
                };
                Ok(flat_op::FlatOp::Update(r))
            }
            Op::AgentActivity(AgentActivity { action, .. }) => {
                let a = &action.hashed.content;
                let r = match &a.data {
                    ActionData::Dna(d) => flat_op::OpActivity::Dna {
                        dna_hash: d.dna_hash.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::AgentValidationPkg(d) => flat_op::OpActivity::AgentValidationPkg {
                        membrane_proof: d.membrane_proof.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::InitZomesComplete(d) => flat_op::OpActivity::InitZomesComplete {
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::OpenChain(d) => flat_op::OpActivity::OpenChain {
                        previous_target: d.prev_target.clone(),
                        close_hash: d.close_hash.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::CloseChain(d) => flat_op::OpActivity::CloseChain {
                        new_target: d.new_target.clone(),
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::CreateLink(d) => {
                        let link_type = activity_link_type(d.zome_index, d.link_type)?;
                        flat_op::OpActivity::CreateLink {
                            link_type,
                            action: TypedAction {
                                header: a.header.clone(),
                                data: d.clone(),
                            },
                        }
                    }
                    ActionData::DeleteLink(d) => flat_op::OpActivity::DeleteLink {
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                    ActionData::Create(d) => {
                        let typed_action = TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        };
                        match activity_entry::<ET>(&d.entry_type)? {
                            ActivityEntry::App { entry_type, .. } => {
                                flat_op::OpActivity::CreateEntry {
                                    app_entry_type: entry_type,
                                    action: typed_action,
                                }
                            }
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                flat_op::OpActivity::CreatePrivateEntry {
                                    app_entry_type: entry_type,
                                    action: typed_action,
                                }
                            }
                            ActivityEntry::Agent => flat_op::OpActivity::CreateAgent {
                                action: typed_action,
                            },
                            ActivityEntry::CapClaim => flat_op::OpActivity::CreateCapClaim {
                                action: typed_action,
                            },
                            ActivityEntry::CapGrant => flat_op::OpActivity::CreateCapGrant {
                                action: typed_action,
                            },
                        }
                    }
                    ActionData::Update(d) => {
                        let typed_action = TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        };
                        match activity_entry::<ET>(&d.entry_type)? {
                            ActivityEntry::App { entry_type, .. } => {
                                flat_op::OpActivity::UpdateEntry {
                                    app_entry_type: entry_type,
                                    action: typed_action,
                                }
                            }
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                flat_op::OpActivity::UpdatePrivateEntry {
                                    app_entry_type: entry_type,
                                    action: typed_action,
                                }
                            }
                            ActivityEntry::Agent => flat_op::OpActivity::UpdateAgent {
                                action: typed_action,
                            },
                            ActivityEntry::CapClaim => flat_op::OpActivity::UpdateCapClaim {
                                action: typed_action,
                            },
                            ActivityEntry::CapGrant => flat_op::OpActivity::UpdateCapGrant {
                                action: typed_action,
                            },
                        }
                    }
                    ActionData::Delete(d) => flat_op::OpActivity::DeleteEntry {
                        action: TypedAction {
                            header: a.header.clone(),
                            data: d.clone(),
                        },
                    },
                };
                Ok(flat_op::FlatOp::AgentActivity(r))
            }
            Op::CreateLink(CreateLink { create_link }) => {
                let a = &create_link.hashed.content;
                let d = match &a.data {
                    ActionData::CreateLink(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "CreateLink op carried a non-CreateLink action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                Ok(flat_op::FlatOp::Link(flat_op::OpLink::CreateLink {
                    link_type,
                    action: TypedAction {
                        header: a.header.clone(),
                        data: d.clone(),
                    },
                }))
            }
            Op::DeleteLink(DeleteLink {
                delete_link,
                create_link,
            }) => {
                let delete_action = &delete_link.hashed.content;
                let delete_data = match &delete_action.data {
                    ActionData::DeleteLink(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "DeleteLink op carried a non-DeleteLink action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let d = match &create_link.data {
                    ActionData::CreateLink(d) => d,
                    other => {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "DeleteLink referenced a non-CreateLink original action: {:?}",
                            other.action_type()
                        ))))
                    }
                };
                let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                Ok(flat_op::FlatOp::Link(flat_op::OpLink::DeleteLink {
                    original_action: TypedAction {
                        header: create_link.header.clone(),
                        data: d.clone(),
                    },
                    link_type,
                    action: TypedAction {
                        header: delete_action.header.clone(),
                        data: delete_data.clone(),
                    },
                }))
            }
            Op::Delete(Delete { delete }) => {
                let action = &delete.hashed.content;
                match &action.data {
                    ActionData::Delete(data) => Ok(flat_op::FlatOp::Delete(flat_op::OpDelete {
                        action: TypedAction {
                            header: action.header.clone(),
                            data: data.clone(),
                        },
                    })),
                    other => Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "Delete op carried a non-Delete action: {:?}",
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
/// To be used only in the context of a CreateEntry authority.
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
/// To be used only in the context of a CreateRecord or AgentActivity authority.
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

/// Maps [`AgentActivity`] ops to their
/// entries. The entry type will be [`None`] if
/// the zome id is not a dependency of this zome.
pub(crate) fn activity_entry<ET>(
    entry_type: &EntryType,
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
        EntryType::AgentPubKey => Ok(ActivityEntry::Agent),
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
    use holochain_integrity_types::prelude::{
        CloseChainData, EntryType, LinkTag, LinkType, MigrationTarget, OpenChainData, RecordEntry,
        Signature, SignedHashed, ZomeIndex,
    };

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
        let op = Op::CreateRecord(CreateRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::CreateRecord(OpRecord::CreateEntry {
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
        let op = Op::CreateRecord(CreateRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::CreateRecord(OpRecord::CreateAgent { .. })
        ));
    }

    #[test]
    fn store_record_dna_flattens_to_dna() {
        types();
        let signed = signed_from_data(ActionData::Dna(DnaData {
            dna_hash: DnaHash::from_raw_36(vec![4u8; 36]),
        }));
        let record = Record::new(signed, RecordEntry::NA);
        let op = Op::CreateRecord(CreateRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(flat, FlatOp::CreateRecord(OpRecord::Dna { .. })));
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
        let op = Op::CreateRecord(CreateRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::CreateRecord(OpRecord::CreateLink {
                link_type: LinkTypes::A,
                ..
            })
        ));
    }

    #[test]
    fn store_record_open_chain_resolves_previous_target() {
        types();
        let target = MigrationTarget::Dna(DnaHash::from_raw_36(vec![11u8; 36]));
        let close = ActionHash::from_raw_36(vec![12u8; 36]);
        let signed = signed_from_data(ActionData::OpenChain(OpenChainData {
            prev_target: target.clone(),
            close_hash: close.clone(),
        }));
        let record = Record::new(signed, RecordEntry::NA);
        let op = Op::CreateRecord(CreateRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        match flat {
            FlatOp::CreateRecord(OpRecord::OpenChain {
                previous_target,
                close_hash,
                ..
            }) => {
                assert_eq!(previous_target, target);
                assert_eq!(close_hash, close);
            }
            other => panic!("expected OpenChain, got {other:?}"),
        }
    }

    #[test]
    fn store_record_close_chain_resolves_new_target() {
        types();
        let target = MigrationTarget::Dna(DnaHash::from_raw_36(vec![13u8; 36]));
        let signed = signed_from_data(ActionData::CloseChain(CloseChainData {
            new_target: Some(target.clone()),
        }));
        let record = Record::new(signed, RecordEntry::NA);
        let op = Op::CreateRecord(CreateRecord { record });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        match flat {
            FlatOp::CreateRecord(OpRecord::CloseChain { new_target, .. }) => {
                assert_eq!(new_target, Some(target));
            }
            other => panic!("expected CloseChain, got {other:?}"),
        }
    }

    #[test]
    fn store_entry_create_app_flattens_to_create_entry() {
        types();
        let signed = signed_from_data(create_app_data());
        let op = Op::CreateEntry(CreateEntry {
            action: signed,
            entry: e(A {}),
        });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::CreateEntry(OpEntry::CreateEntry {
                app_entry: EntryTypes::A(A {}),
                ..
            })
        ));
    }

    #[test]
    fn register_agent_activity_create_app_flattens_with_unit_type() {
        types();
        let signed = signed_from_data(create_app_data());
        let op = Op::AgentActivity(AgentActivity {
            action: signed,
            cached_entry: None,
        });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(
            flat,
            FlatOp::AgentActivity(OpActivity::CreateEntry {
                app_entry_type: Some(UnitEntryTypes::A),
                ..
            })
        ));
    }

    #[test]
    fn register_agent_activity_open_chain_resolves_previous_target() {
        types();
        let target = MigrationTarget::Dna(DnaHash::from_raw_36(vec![14u8; 36]));
        let close = ActionHash::from_raw_36(vec![15u8; 36]);
        let signed = signed_from_data(ActionData::OpenChain(OpenChainData {
            prev_target: target.clone(),
            close_hash: close.clone(),
        }));
        let op = Op::AgentActivity(AgentActivity {
            action: signed,
            cached_entry: None,
        });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        match flat {
            FlatOp::AgentActivity(OpActivity::OpenChain {
                previous_target,
                close_hash,
                ..
            }) => {
                assert_eq!(previous_target, target);
                assert_eq!(close_hash, close);
            }
            other => panic!("expected OpenChain, got {other:?}"),
        }
    }

    #[test]
    fn register_agent_activity_close_chain_resolves_new_target() {
        types();
        let target = MigrationTarget::Dna(DnaHash::from_raw_36(vec![16u8; 36]));
        let signed = signed_from_data(ActionData::CloseChain(CloseChainData {
            new_target: Some(target.clone()),
        }));
        let op = Op::AgentActivity(AgentActivity {
            action: signed,
            cached_entry: None,
        });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        match flat {
            FlatOp::AgentActivity(OpActivity::CloseChain { new_target, .. }) => {
                assert_eq!(new_target, Some(target));
            }
            other => panic!("expected CloseChain, got {other:?}"),
        }
    }

    #[test]
    fn register_delete_flattens_to_register_delete() {
        types();
        let signed = signed_from_data(ActionData::Delete(DeleteData {
            deletes_address: ActionHash::from_raw_36(vec![7u8; 36]),
            deletes_entry_address: EntryHash::from_raw_36(vec![8u8; 36]),
        }));
        let op = Op::Delete(Delete { delete: signed });
        let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
        assert!(matches!(flat, FlatOp::Delete(_)));
    }
}
