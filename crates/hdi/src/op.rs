//! Helper types for working with [`Op`]s
use crate::prelude::*;

#[cfg(test)]
mod test;

/// This trait provides a conversion to a convenience type [`FlatOp`]
/// for use in the validation call back.
///
/// Not all data is available in the [`FlatOp`]. This is why the [`Op`]
/// is not taken by value and can still be used after this conversion.
///
/// There is data that is common to all ops and can be accessed via helpers on
/// the op.
/// - Get the [`Op::author()`] of the op.
/// - Get the [`Op::timestamp()`] for when the op was created.
/// - Get the [`Op::action_seq()`] of the op.
/// - Get the [`Op::prev_action()`] of the op.
/// - Get the [`Op::action_type()`] of the op.
pub trait OpHelper {
    /// Converts an [`Op`] to a [`FlatOp`] without consuming it.
    /// This will clone the required internal data.
    fn flattened<ET, LT>(&self) -> Result<FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>;

    /// Alias for `flattened`, for backward compatibility
    #[deprecated = "`to_type` has been renamed to `flattened`, please use that name instead"]
    fn to_type<ET, LT>(&self) -> Result<FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>,
    {
        self.flattened()
    }
}

/// [`RecordEntry`]s that takes a reference.
enum RecordEntryRef<'a> {
    Present(&'a Entry),
    Hidden,
    NotApplicable,
    NotStored,
}

/// All possible variants that an [`RegisterAgentActivity`]
/// with an [`Action`] that has an [`EntryType`] can produce.
#[derive(Debug)]
enum ActivityEntry<Unit> {
    App { entry_type: Option<Unit> },
    PrivateApp { entry_type: Option<Unit> },
    Agent(AgentPubKey),
    CapClaim(EntryHash),
    CapGrant(EntryHash),
}

impl OpHelper for Op {
    fn flattened<ET, LT>(&self) -> Result<FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>,
    {
        match self {
            Op::StoreRecord(StoreRecord { record }) => {
                let r = match record.action() {
                    Action::Dna(action) => OpRecord::Dna {
                        dna_hash: action.hash.clone(),
                        action: action.clone(),
                    },
                    Action::AgentValidationPkg(action) => {
                        let AgentValidationPkg { membrane_proof, .. } = action;
                        OpRecord::AgentValidationPkg {
                            membrane_proof: membrane_proof.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::InitZomesComplete(action) => OpRecord::InitZomesComplete {
                        action: action.clone(),
                    },
                    Action::CreateLink(action) => {
                        let CreateLink {
                            zome_index,
                            link_type,
                            base_address,
                            target_address,
                            tag,
                            ..
                        } = action;
                        let link_type = in_scope_link_type(*zome_index, *link_type)?;
                        OpRecord::CreateLink {
                            base_address: base_address.clone(),
                            target_address: target_address.clone(),
                            tag: tag.clone(),
                            link_type,
                            action: action.clone(),
                        }
                    }
                    Action::DeleteLink(action) => {
                        let DeleteLink {
                            base_address,
                            link_add_address,
                            ..
                        } = action;
                        OpRecord::DeleteLink {
                            original_action_hash: link_add_address.clone(),
                            base_address: base_address.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::OpenChain(action) => {
                        let OpenChain { prev_dna_hash, .. } = action;
                        OpRecord::OpenChain {
                            previous_dna_hash: prev_dna_hash.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::CloseChain(action) => {
                        let CloseChain { new_dna_hash, .. } = action;
                        OpRecord::CloseChain {
                            new_dna_hash: new_dna_hash.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::Create(action) => {
                        let Create {
                            entry_type,
                            entry_hash,
                            ..
                        } = action;
                        match entry_type {
                            EntryType::AgentPubKey => OpRecord::CreateAgent {
                                agent: entry_hash.clone().into(),
                                action: action.clone(),
                            },
                            EntryType::App(entry_def) => {
                                match get_app_entry_type_for_record_authority(
                                    entry_def,
                                    record.entry.as_option(),
                                )? {
                                    UnitEnumEither::Enum(app_entry) => OpRecord::CreateEntry {
                                        app_entry,
                                        action: action.clone(),
                                    },
                                    UnitEnumEither::Unit(app_entry_type) => {
                                        OpRecord::CreatePrivateEntry {
                                            app_entry_type,
                                            action: action.clone(),
                                        }
                                    }
                                }
                            }
                            EntryType::CapClaim => OpRecord::CreateCapClaim {
                                action: action.clone(),
                            },
                            EntryType::CapGrant => OpRecord::CreateCapGrant {
                                action: action.clone(),
                            },
                        }
                    }
                    Action::Update(action) => {
                        let Update {
                            entry_type,
                            entry_hash,
                            original_action_address: original_action_hash,
                            original_entry_address: original_entry_hash,
                            ..
                        } = action;
                        match entry_type {
                            EntryType::AgentPubKey => OpRecord::UpdateAgent {
                                original_key: original_entry_hash.clone().into(),
                                original_action_hash: original_action_hash.clone(),
                                new_key: entry_hash.clone().into(),
                                action: action.clone(),
                            },
                            EntryType::App(entry_def) => {
                                match get_app_entry_type_for_record_authority(
                                    entry_def,
                                    record.entry.as_option(),
                                )? {
                                    UnitEnumEither::Enum(app_entry) => OpRecord::UpdateEntry {
                                        original_action_hash: original_action_hash.clone(),
                                        original_entry_hash: original_entry_hash.clone(),
                                        app_entry,
                                        action: action.clone(),
                                    },
                                    UnitEnumEither::Unit(app_entry_type) => {
                                        OpRecord::UpdatePrivateEntry {
                                            original_action_hash: original_action_hash.clone(),
                                            original_entry_hash: original_entry_hash.clone(),
                                            app_entry_type,
                                            action: action.clone(),
                                        }
                                    }
                                }
                            }
                            EntryType::CapClaim => OpRecord::UpdateCapClaim {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                action: action.clone(),
                            },
                            EntryType::CapGrant => OpRecord::UpdateCapGrant {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                action: action.clone(),
                            },
                        }
                    }
                    Action::Delete(action) => {
                        let Delete {
                            deletes_address,
                            deletes_entry_address,
                            ..
                        } = action;
                        OpRecord::DeleteEntry {
                            original_action_hash: deletes_address.clone(),
                            original_entry_hash: deletes_entry_address.clone(),
                            action: action.clone(),
                        }
                    }
                };
                Ok(FlatOp::StoreRecord(r))
            }
            Op::StoreEntry(StoreEntry { action, entry }) => {
                let r = match &action.hashed.content {
                    EntryCreationAction::Create(action) => {
                        let Create {
                            entry_type,
                            entry_hash,
                            ..
                        } = action;
                        match entry_type {
                            EntryType::AgentPubKey => OpEntry::CreateAgent {
                                agent: entry_hash.clone().into(),
                                action: action.clone(),
                            },
                            EntryType::App(app_entry) => OpEntry::CreateEntry {
                                app_entry: get_app_entry_type_for_store_entry_authority(app_entry, entry)?,
                                action: action.clone(),
                            },
                            EntryType::CapClaim => OpEntry::CreateCapClaim {
                                entry: match entry {
                                    Entry::CapClaim(entry) => entry.clone(),
                                    _ => return Err(wasm_error!(WasmErrorInner::Guest(format!("Entry type does not match. CapClaim expected but got: {:?}", entry))))
                                },
                                action: action.clone(),
                            },
                            EntryType::CapGrant => OpEntry::CreateCapGrant {
                                entry: match entry {
                                    Entry::CapGrant(entry) => entry.clone(),
                                    _ => return Err(wasm_error!(WasmErrorInner::Guest(format!("Entry type does not match. CapGrant expected but got: {:?}", entry))))
                                },
                                action: action.clone(),
                            },
                        }
                    }
                    EntryCreationAction::Update(action) => {
                        let Update {
                            original_action_address: original_action_hash,
                            original_entry_address: original_entry_hash,
                            entry_type,
                            entry_hash,
                            ..
                        } = action;
                        match entry_type {
                            EntryType::AgentPubKey => OpEntry::UpdateAgent {
                                original_key: original_entry_hash.clone().into(),
                                original_action_hash: original_action_hash.clone(),
                                new_key: entry_hash.clone().into(),
                                action: action.clone(),
                            },
                            EntryType::App(entry_def) => {
                                let app_entry = get_app_entry_type_for_store_entry_authority(entry_def, entry)?;
                                OpEntry::UpdateEntry {
                                    original_action_hash: original_action_hash.clone(),
                                    original_entry_hash: original_entry_hash.clone(),
                                    app_entry,
                                    action: action.clone(),
                                }
                            }
                            EntryType::CapClaim => OpEntry::UpdateCapClaim {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                entry: match entry {
                                    Entry::CapClaim(entry) => entry.clone(),
                                    _ => return Err(wasm_error!(WasmErrorInner::Guest(format!("Entry type does not match. CapClaim expected but got: {:?}", entry))))
                                },
                                action: action.clone(),
                            },
                            EntryType::CapGrant => OpEntry::UpdateCapGrant {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                entry: match entry {
                                    Entry::CapGrant(entry) => entry.clone(),
                                    _ => return Err(wasm_error!(WasmErrorInner::Guest(format!("Entry type does not match. CapGrant expected but got: {:?}", entry))))
                                },
                                action: action.clone(),
                            },
                        }
                    }
                };
                Ok(FlatOp::StoreEntry(r))
            }
            Op::RegisterUpdate(RegisterUpdate {
                update,
                new_entry,
                original_entry,
                original_action,
            }) => {
                let Update {
                    original_action_address: original_action_hash,
                    original_entry_address: original_entry_hash,
                    entry_type,
                    entry_hash,
                    ..
                } = &update.hashed.content;
                if original_action.entry_type() != entry_type {
                    return Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "New entry type {:?} doesn't match original entry type {:?}",
                        entry_type,
                        original_action.entry_type()
                    ))));
                }
                let update = match entry_type {
                    EntryType::AgentPubKey => OpUpdate::Agent {
                        original_key: original_entry_hash.clone().into(),
                        original_action_hash: original_action_hash.clone(),
                        new_key: entry_hash.clone().into(),
                        action: update.hashed.content.clone(),
                    },
                    EntryType::App(entry_def) => {
                        let old = get_app_entry_type_for_record_authority::<ET>(
                            entry_def,
                            original_entry.as_ref(),
                        )?;
                        let new = get_app_entry_type_for_record_authority::<ET>(
                            entry_def,
                            new_entry.as_ref(),
                        )?;
                        match (old, new) {
                            (UnitEnumEither::Enum(old), UnitEnumEither::Enum(new)) => {
                                OpUpdate::Entry {
                                    original_action: original_action.clone(),
                                    app_entry: new,
                                    original_app_entry: old,
                                    action: update.hashed.content.clone(),
                                }
                            }
                            (UnitEnumEither::Unit(old), UnitEnumEither::Unit(new)) => {
                                OpUpdate::PrivateEntry {
                                    original_action_hash: original_action_hash.clone(),
                                    app_entry_type: new,
                                    original_app_entry_type: old,
                                    action: update.hashed.content.clone(),
                                }
                            }
                            (_, _) => {
                                return Err(wasm_error!(WasmErrorInner::Guest(format!(
                                    "Attempting to update a private entry to a public entry, or vice versa. old: {:?} new: {:?}",
                                    original_action.entry_type(),
                                    entry_type,
                                ))))
                            }
                        }
                    }
                    EntryType::CapClaim => OpUpdate::CapClaim {
                        original_action_hash: original_action_hash.clone(),
                        action: update.hashed.content.clone(),
                    },
                    EntryType::CapGrant => OpUpdate::CapGrant {
                        original_action_hash: original_action_hash.clone(),
                        action: update.hashed.content.clone(),
                    },
                };
                Ok(FlatOp::RegisterUpdate(update))
            }
            Op::RegisterAgentActivity(RegisterAgentActivity { action, .. }) => {
                let r = match &action.hashed.content {
                    Action::Dna(action) => {
                        let Dna { hash, .. } = action;
                        OpActivity::Dna {
                            dna_hash: hash.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::AgentValidationPkg(action) => {
                        let AgentValidationPkg { membrane_proof, .. } = action;
                        OpActivity::AgentValidationPkg {
                            membrane_proof: membrane_proof.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::InitZomesComplete(action) => OpActivity::InitZomesComplete {
                        action: action.clone(),
                    },
                    Action::OpenChain(action) => {
                        let OpenChain { prev_dna_hash, .. } = action;
                        OpActivity::OpenChain {
                            previous_dna_hash: prev_dna_hash.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::CloseChain(action) => {
                        let CloseChain { new_dna_hash, .. } = action;
                        OpActivity::CloseChain {
                            new_dna_hash: new_dna_hash.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::CreateLink(action) => {
                        let CreateLink {
                            base_address,
                            target_address,
                            zome_index,
                            link_type,
                            tag,
                            ..
                        } = action;
                        let link_type = activity_link_type(*zome_index, *link_type)?;
                        OpActivity::CreateLink {
                            base_address: base_address.clone(),
                            target_address: target_address.clone(),
                            tag: tag.clone(),
                            link_type,
                            action: action.clone(),
                        }
                    }
                    Action::DeleteLink(action) => {
                        let DeleteLink {
                            link_add_address,
                            base_address,
                            ..
                        } = action;
                        OpActivity::DeleteLink {
                            original_action_hash: link_add_address.clone(),
                            base_address: base_address.clone(),
                            action: action.clone(),
                        }
                    }
                    Action::Create(action) => {
                        let Create {
                            entry_type,
                            entry_hash,
                            ..
                        } = action;
                        match activity_entry::<ET>(entry_type, entry_hash)? {
                            ActivityEntry::App { entry_type, .. } => OpActivity::CreateEntry {
                                app_entry_type: entry_type,
                                action: action.clone(),
                            },
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                OpActivity::CreatePrivateEntry {
                                    app_entry_type: entry_type,
                                    action: action.clone(),
                                }
                            }
                            ActivityEntry::Agent(agent) => OpActivity::CreateAgent {
                                agent,
                                action: action.clone(),
                            },
                            ActivityEntry::CapClaim(_hash) => OpActivity::CreateCapClaim {
                                action: action.clone(),
                            },
                            ActivityEntry::CapGrant(_hash) => OpActivity::CreateCapGrant {
                                action: action.clone(),
                            },
                        }
                    }
                    Action::Update(action) => {
                        let Update {
                            original_action_address,
                            original_entry_address,
                            entry_type,
                            entry_hash,
                            ..
                        } = action;
                        match activity_entry::<ET>(entry_type, entry_hash)? {
                            ActivityEntry::App { entry_type, .. } => OpActivity::UpdateEntry {
                                original_action_hash: original_action_address.clone(),
                                original_entry_hash: original_entry_address.clone(),
                                app_entry_type: entry_type,
                                action: action.clone(),
                            },
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                OpActivity::UpdatePrivateEntry {
                                    original_action_hash: original_action_address.clone(),
                                    original_entry_hash: original_entry_address.clone(),
                                    app_entry_type: entry_type,
                                    action: action.clone(),
                                }
                            }
                            ActivityEntry::Agent(new_key) => OpActivity::UpdateAgent {
                                original_action_hash: original_action_address.clone(),
                                original_key: original_entry_address.clone().into(),
                                new_key,
                                action: action.clone(),
                            },
                            ActivityEntry::CapClaim(_entry_hash) => OpActivity::UpdateCapClaim {
                                original_action_hash: original_action_address.clone(),
                                original_entry_hash: original_entry_address.clone(),
                                action: action.clone(),
                            },
                            ActivityEntry::CapGrant(_entry_hash) => OpActivity::UpdateCapGrant {
                                original_action_hash: original_action_address.clone(),
                                original_entry_hash: original_entry_address.clone(),
                                action: action.clone(),
                            },
                        }
                    }
                    Action::Delete(action) => {
                        let Delete {
                            deletes_address,
                            deletes_entry_address,
                            ..
                        } = action;
                        OpActivity::DeleteEntry {
                            original_action_hash: deletes_address.clone(),
                            original_entry_hash: deletes_entry_address.clone(),
                            action: action.clone(),
                        }
                    }
                };
                Ok(FlatOp::RegisterAgentActivity(r))
            }
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                let CreateLink {
                    base_address,
                    target_address,
                    zome_index,
                    link_type,
                    tag,
                    ..
                } = &create_link.hashed.content;
                let link_type = in_scope_link_type(*zome_index, *link_type)?;
                Ok(FlatOp::RegisterCreateLink {
                    base_address: base_address.clone(),
                    target_address: target_address.clone(),
                    tag: tag.clone(),
                    link_type,
                    action: create_link.hashed.content.clone(),
                })
            }
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link,
                create_link,
            }) => {
                let CreateLink {
                    base_address,
                    target_address,
                    zome_index,
                    link_type,
                    tag,
                    ..
                } = create_link;
                let link_type = in_scope_link_type(*zome_index, *link_type)?;
                Ok(FlatOp::RegisterDeleteLink {
                    original_action: create_link.clone(),
                    base_address: base_address.clone(),
                    target_address: target_address.clone(),
                    tag: tag.clone(),
                    link_type,
                    action: delete_link.hashed.content.clone(),
                })
            }
            Op::RegisterDelete(RegisterDelete {
                delete,
                original_action,
                original_entry: orig_entry,
            }) => {
                let Delete {
                    deletes_entry_address: original_entry_hash,
                    ..
                } = &delete.hashed.content;
                let r = match original_action.entry_type() {
                    EntryType::AgentPubKey => OpDelete::Agent {
                        original_action: original_action.clone(),
                        original_key: original_entry_hash.clone().into(),
                        action: delete.hashed.content.clone(),
                    },
                    EntryType::App(original_entry_type) => {
                        match get_app_entry_type_for_record_authority::<ET>(
                            original_entry_type,
                            orig_entry.as_ref(),
                        )? {
                            UnitEnumEither::Enum(original_app_entry) => OpDelete::Entry {
                                original_action: original_action.clone(),
                                original_app_entry,
                                action: delete.hashed.content.clone(),
                            },
                            UnitEnumEither::Unit(original_app_entry_type) => {
                                OpDelete::PrivateEntry {
                                    original_action: original_action.clone(),
                                    original_app_entry_type,
                                    action: delete.hashed.content.clone(),
                                }
                            }
                        }
                    }
                    EntryType::CapClaim => OpDelete::CapClaim {
                        original_action: original_action.clone(),
                        action: delete.hashed.content.clone(),
                    },
                    EntryType::CapGrant => OpDelete::CapGrant {
                        original_action: original_action.clone(),
                        action: delete.hashed.content.clone(),
                    },
                };
                Ok(FlatOp::RegisterDelete(r))
            }
        }
    }
}

/// Produces the user-defined entry type enum. Even if the entry is private, this will succeed.
/// To be used only in the context of a StoreEntry authority.
fn get_app_entry_type_for_store_entry_authority<ET>(
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
fn get_app_entry_type_for_record_authority<ET>(
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
fn activity_entry<ET>(
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
        EntryType::CapClaim => Ok(ActivityEntry::CapClaim(entry_hash.clone())),
        EntryType::CapGrant => Ok(ActivityEntry::CapGrant(entry_hash.clone())),
    }
}

/// Get the app defined link type from a [`ZomeIndex`] and [`LinkType`].
/// If the [`ZomeIndex`] is not a dependency of this zome then return a host error.
fn in_scope_link_type<LT>(zome_index: ZomeIndex, link_type: LinkType) -> Result<LT, WasmError>
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
fn activity_link_type<LT>(
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
                    "Entry type: {:?} is out of range for this zome.",
                    entry_def_index
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

impl<'a> From<&'a RecordEntry> for RecordEntryRef<'a> {
    fn from(r: &'a RecordEntry) -> Self {
        match r {
            RecordEntry::Present(e) => RecordEntryRef::Present(e),
            RecordEntry::Hidden => RecordEntryRef::Hidden,
            RecordEntry::NotApplicable => RecordEntryRef::NotApplicable,
            RecordEntry::NotStored => RecordEntryRef::NotStored,
        }
    }
}
