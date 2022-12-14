//! Helper types for working with [`Op`]s
use crate::prelude::*;

#[cfg(test)]
mod test;

/// This trait provides a conversion to a convenience type [`OpType`]
/// for use in the validation call back.
///
/// Not all data is available in the [`OpType`]. This is why the [`Op`]
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
    /// Converts an [`Op`] to an [`OpType`] without consuming it.
    /// This will clone the required internal data.
    fn to_type<ET, LT>(&self) -> Result<OpType<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>;
}

#[derive(Debug)]
/// All the possible variants for entries
/// that are in scope for a zome.
enum InScopeEntry<ET>
where
    ET: UnitEnum,
{
    Agent(AgentPubKey),
    App(ET),
    PrivateApp(<ET as UnitEnum>::Unit),
    CapClaim,
    CapGrant,
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
    fn to_type<ET, LT>(&self) -> Result<OpType<ET, LT>, WasmError>
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
                        map_entry(entry_type, entry_hash, (&record.entry).into())?
                            .into_op_record(action)?
                    }
                    Action::Update(action) => {
                        let Update {
                            entry_type,
                            entry_hash,
                            original_action_address: original_action_hash,
                            original_entry_address: original_entry_hash,
                            ..
                        } = action;
                        match map_entry::<ET>(entry_type, entry_hash, (&record.entry).into())? {
                            InScopeEntry::App(entry_type) => OpRecord::UpdateEntry {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                app_entry: entry_type,
                                action: action.clone(),
                            },
                            InScopeEntry::PrivateApp(entry_type) => OpRecord::UpdatePrivateEntry {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                app_entry_type: entry_type,
                                action: action.clone(),
                            },
                            InScopeEntry::Agent(new_key) => OpRecord::UpdateAgent {
                                original_agent: original_entry_hash.clone().into(),
                                original_action_hash: original_action_hash.clone(),
                                agent: new_key,
                                action: action.clone(),
                            },
                            InScopeEntry::CapClaim => OpRecord::UpdateCapClaim {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                action: action.clone(),
                            },
                            InScopeEntry::CapGrant => OpRecord::UpdateCapGrant {
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
                Ok(OpType::StoreRecord(r))
            }
            Op::StoreEntry(StoreEntry { action, entry }) => {
                let r = match &action.hashed.content {
                    EntryCreationAction::Create(Create {
                        entry_type,
                        entry_hash,
                        ..
                    }) => store_entry_create(entry_type, entry_hash, entry, action)?,
                    EntryCreationAction::Update(action) => {
                        let Update {
                            original_action_address: original_action_hash,
                            original_entry_address: original_entry_hash,
                            entry_type,
                            entry_hash,
                            ..
                        } = action;
                        match map_entry::<ET>(
                            entry_type,
                            entry_hash,
                            RecordEntryRef::Present(entry),
                        )? {
                            InScopeEntry::App(entry_type) => OpEntry::UpdateEntry {
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                app_entry: entry_type,
                                action: action.clone(),
                            },
                            InScopeEntry::Agent(agent_key) => OpEntry::UpdateAgent {
                                original_key: original_entry_hash.clone().into(),
                                original_action_hash: original_action_hash.clone(),
                                new_key: agent_key,
                                action: action.clone(),
                            },
                            _ => {
                                return Err(wasm_error!(WasmErrorInner::Guest(
                                    "StoreEntry should not exist for private entries Id"
                                        .to_string()
                                )))
                            }
                        }
                    }
                };
                Ok(OpType::StoreEntry(r))
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
                if original_action.entry_type() != entry_type
                    && ((new_entry.is_some() && original_entry.is_some())
                        || (new_entry.is_none() && original_entry.is_none()))
                {
                    return Err(wasm_error!(WasmErrorInner::Guest(format!(
                        "New entry type {:?} doesn't match original entry type {:?}",
                        entry_type,
                        original_action.entry_type()
                    ))));
                }
                let new_entry = new_entry
                    .as_ref()
                    .map_or(RecordEntryRef::Hidden, RecordEntryRef::Present);
                let original_entry = original_entry
                    .as_ref()
                    .map_or(RecordEntryRef::Hidden, RecordEntryRef::Present);
                let r = match map_entry::<ET>(original_action.entry_type(), entry_hash, new_entry)?
                {
                    InScopeEntry::Agent(new_key) => Some(OpUpdate::Agent {
                        original_key: original_entry_hash.clone().into(),
                        original_action_hash: original_action_hash.clone(),
                        new_key,
                        action: update.hashed.content.clone(),
                    }),
                    InScopeEntry::App(new_entry_type) => {
                        match map_entry::<ET>(entry_type, entry_hash, original_entry)? {
                            InScopeEntry::App(original_entry_type) => Some(OpUpdate::Entry {
                                original_action_hash: original_action_hash.clone(),
                                app_entry: new_entry_type,
                                original_app_entry: original_entry_type,
                                action: update.hashed.content.clone(),
                            }),
                            _ => None,
                        }
                    }
                    InScopeEntry::PrivateApp(new_entry_type) => {
                        match map_entry::<ET>(entry_type, entry_hash, original_entry)? {
                            InScopeEntry::PrivateApp(original_entry_type) => {
                                Some(OpUpdate::PrivateEntry {
                                    original_action_hash: original_action_hash.clone(),
                                    app_entry_type: new_entry_type,
                                    original_app_entry_type: original_entry_type,
                                    action: update.hashed.content.clone(),
                                })
                            }
                            _ => None,
                        }
                    }
                    InScopeEntry::CapClaim => Some(OpUpdate::CapClaim {
                        original_action_hash: original_action_hash.clone(),
                        action: update.hashed.content.clone(),
                    }),
                    InScopeEntry::CapGrant => Some(OpUpdate::CapGrant {
                        original_action_hash: original_action_hash.clone(),
                        action: update.hashed.content.clone(),
                    }),
                };
                match r {
                    Some(r) => Ok(OpType::RegisterUpdate(r)),
                    None => unreachable!("As entry types are already checked to match"),
                }
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
                Ok(OpType::RegisterAgentActivity(r))
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
                Ok(OpType::RegisterCreateLink {
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
                Ok(OpType::RegisterDeleteLink {
                    original_link_action_hash: delete_link.hashed.link_add_address.clone(),
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
                let r = match map_entry::<ET>(
                    original_action.entry_type(),
                    original_entry_hash,
                    orig_entry
                        .as_ref()
                        .map_or(RecordEntryRef::Hidden, RecordEntryRef::Present),
                )? {
                    InScopeEntry::Agent(_) => OpDelete::Agent {
                        original_action: original_action.clone(),
                        original_key: original_entry_hash.clone().into(),
                        action: delete.hashed.content.clone(),
                    },
                    InScopeEntry::App(original_entry_type) => OpDelete::Entry {
                        original_action: original_action.clone(),
                        original_app_entry: original_entry_type,
                        action: delete.hashed.content.clone(),
                    },
                    InScopeEntry::PrivateApp(original_entry_type) => OpDelete::PrivateEntry {
                        original_action: original_action.clone(),
                        original_app_entry_type: original_entry_type,
                        action: delete.hashed.content.clone(),
                    },
                    InScopeEntry::CapClaim => OpDelete::CapClaim {
                        original_action: original_action.clone(),
                        action: delete.hashed.content.clone(),
                    },
                    InScopeEntry::CapGrant => OpDelete::CapGrant {
                        original_action: original_action.clone(),
                        action: delete.hashed.content.clone(),
                    },
                };
                Ok(OpType::RegisterDelete(r))
            }
        }
    }
}

fn store_entry_create<ET>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
    entry: &Entry,
    action: &SignedHashed<EntryCreationAction>,
) -> Result<OpEntry<ET>, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
{
    match map_entry::<ET>(entry_type, entry_hash, RecordEntryRef::Present(entry))? {
        InScopeEntry::App(entry_type) => Ok(OpEntry::CreateEntry {
            app_entry: entry_type,
            action: action.hashed.content.clone(),
        }),
        InScopeEntry::Agent(agent_key) => Ok(OpEntry::CreateAgent {
            agent: agent_key,
            action: action.hashed.content.clone(),
        }),
        _ => Err(wasm_error!(WasmErrorInner::Guest(
            "StoreEntry should not exist for private entries Id".to_string()
        ))),
    }
}

/// Maps an entry type and entry to an
/// [`InScopeEntry`]. This will return a guest error
/// and invalidate the op if the zome id is this zome but
/// entry type is not in scope.
fn map_entry<ET>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
    entry: RecordEntryRef,
) -> Result<InScopeEntry<ET>, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
{
    match entry {
        RecordEntryRef::Present(entry) => match entry_type {
            EntryType::App(AppEntryDef {
                zome_index,
                entry_index: entry_def_index,
                visibility: EntryVisibility::Public,
                ..
            }) => {
                if !matches!(entry, Entry::App(_)) {
                    return Err(wasm_error!(WasmErrorInner::Guest(
                        "Entry type is App but Entry is not App".to_string()
                    )));
                }
                let entry_type = <ET as EntryTypesHelper>::deserialize_from_type(
                    *zome_index,
                    *entry_def_index,
                    entry,
                )?;
                match entry_type {
                    Some(entry_type) => Ok(InScopeEntry::App(entry_type)),
                    None => Err(deny_other_zome()),
                }
            }
            EntryType::AgentPubKey => {
                if !matches!(entry, Entry::Agent(_)) {
                    return Err(wasm_error!(WasmErrorInner::Guest(
                        "Entry type is AgentPubKey but Entry is not AgentPubKey".to_string()
                    )));
                }
                Ok(InScopeEntry::Agent(entry_hash.clone().into()))
            }
            _ => Err(wasm_error!(WasmErrorInner::Guest(
                "Entry type is a capability and should be private but there is an entry present"
                    .to_string()
            ))),
        },
        RecordEntryRef::Hidden => match entry_type {
            EntryType::App(AppEntryDef {
                zome_index,
                entry_index: entry_def_index,
                visibility: EntryVisibility::Private,
            }) => match get_unit_entry_type::<ET>(*zome_index, *entry_def_index)? {
                Some(unit) => Ok(InScopeEntry::PrivateApp(unit)),
                None => Err(deny_other_zome()),
            },
            EntryType::App(AppEntryDef {
                visibility: EntryVisibility::Public,
                ..
            }) => Err(wasm_error!(WasmErrorInner::Guest(
                "Entry type is public but entry is hidden".to_string()
            ))),
            EntryType::CapClaim => Ok(InScopeEntry::CapClaim),
            EntryType::CapGrant => Ok(InScopeEntry::CapGrant),
            EntryType::AgentPubKey => Err(wasm_error!(WasmErrorInner::Guest(
                "Entry type AgentPubKey is missing entry.".to_string()
            ))),
        },
        RecordEntryRef::NotApplicable => Err(wasm_error!(WasmErrorInner::Guest(
            "Has Entry type but entry is marked not applicable".to_string()
        ))),
        RecordEntryRef::NotStored => match entry_type {
            EntryType::CapClaim | EntryType::CapGrant => Err(wasm_error!(WasmErrorInner::Guest(
                "Capability tokens are never publicly stored.".to_string()
            ))),
            _ => Err(wasm_error!(WasmErrorInner::Host(
                "Has Entry type but the entry is not currently stored.".to_string()
            ))),
        },
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

impl<ET> InScopeEntry<ET>
where
    ET: UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
{
    fn into_op_record<LT>(self, action: &Create) -> Result<OpRecord<ET, LT>, WasmError>
    where
        LT: LinkTypesHelper,
    {
        match self {
            InScopeEntry::Agent(agent) => Ok(OpRecord::CreateAgent {
                agent,
                action: action.clone(),
            }),
            InScopeEntry::App(entry_type) => Ok(OpRecord::CreateEntry {
                app_entry: entry_type,
                action: action.clone(),
            }),
            InScopeEntry::PrivateApp(entry_type) => Ok(OpRecord::CreatePrivateEntry {
                app_entry_type: entry_type,
                action: action.clone(),
            }),
            InScopeEntry::CapClaim => Ok(OpRecord::CreateCapClaim {
                action: action.clone(),
            }),
            InScopeEntry::CapGrant => Ok(OpRecord::CreateCapGrant {
                action: action.clone(),
            }),
        }
    }
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

// impl<Unit, LT> From<ActivityEntry<Unit>> for OpActivity<Unit, LT> {
//     fn from(e: ActivityEntry<Unit>) -> Self {
//         match e {
//             ActivityEntry::App {
//                 entry_hash,
//                 entry_type,
//             } => OpActivity::CreateEntry {
//                 entry_hash,
//                 entry_type,
//             },
//             ActivityEntry::PrivateApp {
//                 entry_hash,
//                 entry_type,
//             } => OpActivity::CreatePrivateEntry {
//                 entry_hash,
//                 entry_type,
//             },
//             ActivityEntry::Agent(key) => OpActivity::CreateAgent(key),
//             ActivityEntry::CapClaim(hash) => OpActivity::CreateCapClaim(hash),
//             ActivityEntry::CapGrant(hash) => OpActivity::CreateCapGrant(hash),
//         }
//     }
// }
