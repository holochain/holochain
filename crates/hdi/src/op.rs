use crate::prelude::*;

pub trait OpHelper {
    fn into_type<ET, LT>(&self) -> Result<OpType<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>;
}

impl OpHelper for Op {
    /// TODO
    fn into_type<ET, LT>(&self) -> Result<OpType<ET, LT>, WasmError>
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
                    Action::Dna(_) => todo!(),
                    Action::AgentValidationPkg(_) => todo!(),
                    Action::InitZomesComplete(_) => todo!(),
                    Action::CreateLink(CreateLink {
                        zome_id,
                        link_type,
                        base_address,
                        target_address,
                        tag,
                        ..
                    }) => match <LT as LinkTypesHelper>::from_type(*zome_id, *link_type)? {
                        Some(link_type) => OpRecord::CreateLink {
                            base_address: base_address.clone(),
                            target_address: target_address.clone(),
                            tag: tag.clone(),
                            link_type,
                        },
                        _ => todo!(),
                    },
                    Action::DeleteLink(DeleteLink {
                        link_add_address, ..
                    }) => OpRecord::DeleteLink(link_add_address.clone()),
                    Action::OpenChain(_) => todo!(),
                    Action::CloseChain(_) => todo!(),
                    Action::Create(Create {
                        entry_type,
                        entry_hash,
                        ..
                    }) => store_record_create(entry_type, entry_hash, &record.entry)?,
                    Action::Update(Update {
                        entry_type,
                        entry_hash,
                        original_action_address: original_action_hash,
                        original_entry_address: original_entry_hash,
                        ..
                    }) => {
                        match store_record_create::<ET, LT>(entry_type, entry_hash, &record.entry)?
                        {
                            OpRecord::CreateEntry {
                                entry_hash,
                                entry_type,
                            } => OpRecord::UpdateEntry {
                                entry_hash,
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                entry_type,
                            },
                            OpRecord::CreatePrivateEntry {
                                entry_hash,
                                entry_type,
                            } => OpRecord::UpdatePrivateEntry {
                                entry_hash,
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                entry_type,
                            },
                            OpRecord::CreateAgent(new_key) => OpRecord::UpdateAgent {
                                original_key: original_entry_hash.clone().into(),
                                original_action_hash: original_action_hash.clone(),
                                new_key,
                            },
                            _ => unreachable!("This record is never created in this arm"),
                        }
                    }
                    Action::Delete(Delete {
                        deletes_address,
                        deletes_entry_address,
                        ..
                    }) => OpRecord::DeleteEntry {
                        original_action_hash: deletes_address.clone(),
                        original_entry_hash: deletes_entry_address.clone(),
                    },
                };
                Ok(OpType::StoreRecord(r))
            }
            Op::StoreEntry(StoreEntry { action, entry }) => {
                let r = match &action.hashed.content {
                    EntryCreationAction::Create(Create {
                        entry_type,
                        entry_hash,
                        ..
                    }) => store_entry_create(entry_type, entry_hash, entry)?,
                    EntryCreationAction::Update(Update {
                        original_action_address: original_action_hash,
                        original_entry_address: original_entry_hash,
                        entry_type,
                        entry_hash,
                        ..
                    }) => match store_entry_create::<ET>(entry_type, entry_hash, entry)? {
                        OpEntry::CreateEntry {
                            entry_hash,
                            entry_type,
                        } => OpEntry::UpdateEntry {
                            entry_hash,
                            original_action_hash: original_action_hash.clone(),
                            original_entry_hash: original_entry_hash.clone(),
                            entry_type,
                        },
                        OpEntry::CreateAgent(new_key) => OpEntry::UpdateAgent {
                            original_key: original_entry_hash.clone().into(),
                            original_action_hash: original_action_hash.clone(),
                            new_key,
                        },
                        _ => unreachable!("This record is never created in this arm"),
                    },
                };
                Ok(OpType::StoreEntry(r))
            }
            Op::RegisterUpdate(RegisterUpdate {
                update,
                new_entry,
                original_entry: orig_entry,
                ..
            }) => {
                let Update {
                    original_action_address: original_action_hash,
                    original_entry_address: original_entry_hash,
                    entry_type,
                    entry_hash,
                    ..
                } = &update.hashed.content;
                let r = match original_entry::<ET>(entry_type, entry_hash, new_entry.as_ref())? {
                    OriginalEntry::Agent(new_key) => Some(OpUpdate::Agent {
                        original_key: original_entry_hash.clone().into(),
                        original_action_hash: original_action_hash.clone(),
                        new_key,
                    }),
                    OriginalEntry::App(new_entry_type) => {
                        match original_entry::<ET>(entry_type, &entry_hash, orig_entry.as_ref())? {
                            OriginalEntry::App(original_entry_type) => Some(OpUpdate::Entry {
                                entry_hash: entry_hash.clone(),
                                original_action_hash: original_action_hash.clone(),
                                original_entry_hash: original_entry_hash.clone(),
                                new_entry_type,
                                original_entry_type,
                            }),
                            _ => None,
                        }
                    }
                    OriginalEntry::PrivateApp(_) => todo!(),
                    OriginalEntry::CapClaim => todo!(),
                    OriginalEntry::CapGrant => todo!(),
                    OriginalEntry::OutOfScope => {
                        todo!("Host error as this should not be called out of scope")
                    }
                };
                match r {
                    Some(r) => Ok(OpType::RegisterUpdate(r)),
                    None => todo!("guest error because types don't match"),
                }
            }
            Op::RegisterAgentActivity(RegisterAgentActivity { action }) => {
                let r = match &action.hashed.content {
                    Action::Dna(_) => todo!(),
                    Action::AgentValidationPkg(_) => todo!(),
                    Action::InitZomesComplete(_) => todo!(),
                    Action::OpenChain(_) => todo!(),
                    Action::CloseChain(_) => todo!(),
                    Action::CreateLink(CreateLink {
                        base_address,
                        target_address,
                        zome_id,
                        link_type,
                        tag,
                        ..
                    }) => {
                        let link_type = <LT as LinkTypesHelper>::from_type(*zome_id, *link_type)?;
                        OpActivity::CreateLink {
                            base_address: base_address.clone(),
                            target_address: target_address.clone(),
                            tag: tag.clone(),
                            link_type,
                        }
                    }
                    Action::DeleteLink(DeleteLink {
                        link_add_address, ..
                    }) => OpActivity::DeleteLink(link_add_address.clone()),
                    Action::Create(Create {
                        entry_type,
                        entry_hash,
                        ..
                    }) => activity_create::<ET, LT>(entry_type, entry_hash)?,
                    Action::Update(Update {
                        original_action_address,
                        original_entry_address,
                        entry_type,
                        entry_hash,
                        ..
                    }) => match activity_create::<ET, LT>(entry_type, entry_hash)? {
                        OpActivity::CreateEntry {
                            entry_hash,
                            entry_type,
                        } => OpActivity::UpdateEntry {
                            entry_hash,
                            original_action_hash: original_action_address.clone(),
                            original_entry_hash: original_entry_address.clone(),
                            entry_type,
                        },
                        OpActivity::CreatePrivateEntry {
                            entry_hash,
                            entry_type,
                        } => OpActivity::UpdatePrivateEntry {
                            entry_hash,
                            original_action_hash: original_action_address.clone(),
                            original_entry_hash: original_entry_address.clone(),
                            entry_type,
                        },
                        OpActivity::CreateAgent(new_key) => OpActivity::UpdateAgent {
                            original_action_hash: original_action_address.clone(),
                            original_key: original_entry_address.clone().into(),
                            new_key,
                        },
                        _ => unreachable!("This action is never created in this arm"),
                    },
                    Action::Delete(Delete {
                        deletes_address,
                        deletes_entry_address,
                        ..
                    }) => OpActivity::DeleteEntry {
                        original_action_hash: deletes_address.clone(),
                        original_entry_hash: deletes_entry_address.clone(),
                    },
                };
                Ok(OpType::RegisterAgentActivity(r))
            }
            Op::RegisterCreateLink(RegisterCreateLink { create_link }) => {
                let CreateLink {
                    base_address,
                    target_address,
                    zome_id,
                    link_type,
                    tag,
                    ..
                } = &create_link.hashed.content;
                match <LT as LinkTypesHelper>::from_type(*zome_id, *link_type)? {
                    Some(link_type) => Ok(OpType::RegisterCreateLink {
                        base_address: base_address.clone(),
                        target_address: target_address.clone(),
                        tag: tag.clone(),
                        link_type,
                    }),
                    _ => todo!(),
                }
            }
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link,
                create_link,
            }) => {
                let CreateLink {
                    base_address,
                    target_address,
                    zome_id,
                    link_type,
                    tag,
                    ..
                } = create_link;
                match <LT as LinkTypesHelper>::from_type(*zome_id, *link_type)? {
                    Some(link_type) => Ok(OpType::RegisterDeleteLink {
                        original_link_hash: delete_link.hashed.link_add_address.clone(),
                        base_address: base_address.clone(),
                        target_address: target_address.clone(),
                        tag: tag.clone(),
                        link_type,
                    }),
                    _ => todo!(),
                }
            }
            Op::RegisterDelete(RegisterDelete {
                delete,
                original_action,
                original_entry: orig_entry,
            }) => {
                let Delete {
                    deletes_address: original_action_hash,
                    deletes_entry_address: original_entry_hash,
                    ..
                } = &delete.hashed.content;
                let r = match original_entry::<ET>(
                    original_action.entry_type(),
                    original_entry_hash,
                    orig_entry.as_ref(),
                )? {
                    OriginalEntry::Agent(_) => OpDelete::Agent {
                        original_key: original_entry_hash.clone().into(),
                        original_action_hash: original_action_hash.clone(),
                    },
                    OriginalEntry::App(original_entry_type) => OpDelete::Entry {
                        original_action_hash: original_action_hash.clone(),
                        original_entry_hash: original_entry_hash.clone(),
                        original_entry_type,
                    },
                    OriginalEntry::PrivateApp(_) => todo!(),
                    OriginalEntry::CapClaim => todo!(),
                    OriginalEntry::CapGrant => todo!(),
                    OriginalEntry::OutOfScope => todo!(),
                };
                Ok(OpType::RegisterDelete(r))
            }
        }
    }
}

fn store_record_create<ET, LT>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
    entry: &RecordEntry,
) -> Result<OpRecord<ET, LT>, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    LT: LinkTypesHelper,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
    WasmError: From<<LT as LinkTypesHelper>::Error>,
{
    match entry {
        RecordEntry::Present(entry) => match entry_type {
            EntryType::App(AppEntryType {
                zome_id,
                id: entry_def_index,
                ..
            }) => {
                let entry_type = <ET as EntryTypesHelper>::deserialize_from_type(
                    *zome_id,
                    *entry_def_index,
                    entry,
                )?;
                match entry_type {
                    Some(entry_type) => Ok(OpRecord::CreateEntry{entry_hash: entry_hash.clone(), entry_type}),
                    None => todo!("Make into wasm host error as this should never be called with the wrong zome id"),
                }
            }
            EntryType::AgentPubKey => Ok(OpRecord::CreateAgent(entry_hash.clone().into())),
            EntryType::CapClaim | EntryType::CapGrant => {
                todo!("Make guest error because this is malformed")
            }
        },
        RecordEntry::Hidden => match entry_type {
            EntryType::App(AppEntryType {
                zome_id,
                id: entry_def_index,
                ..
            }) => {
                let unit = get_unit_entry_type::<ET>(*zome_id, *entry_def_index)?.ok_or_else(||{
                    wasm_error!(WasmErrorInner::Host(format!(
                        "StoreRecord should not be called with the wrong ZomeId {}. This is a holochain bug",
                        zome_id
                    )))
                })?;
                Ok(OpRecord::CreatePrivateEntry {
                    entry_hash: entry_hash.clone(),
                    entry_type: unit,
                })
            }
            EntryType::AgentPubKey | EntryType::CapClaim | EntryType::CapGrant => {
                todo!("Make guest error because this is malformed")
            }
        },
        RecordEntry::NotApplicable => todo!("Guest error as malformed Record"),
        RecordEntry::NotStored => todo!("Host error as this should be stored"),
    }
}

fn store_entry_create<ET>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
    entry: &Entry,
) -> Result<OpEntry<ET>, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
{
    match entry_type {
        EntryType::App(AppEntryType {
            zome_id,
            id: entry_def_index,
            ..
        }) => {
            let entry_type =
                <ET as EntryTypesHelper>::deserialize_from_type(*zome_id, *entry_def_index, entry)?;
            match entry_type {
                Some(entry_type) => Ok(OpEntry::CreateEntry{entry_hash: entry_hash.clone(), entry_type}),
                None => todo!("Make into wasm host error as this should never be called with the wrong zome id"),
            }
        }
        EntryType::AgentPubKey => Ok(OpEntry::CreateAgent(entry_hash.clone().into())),
        EntryType::CapClaim | EntryType::CapGrant => {
            todo!("Make guest error because this is malformed")
        }
    }
}

enum OriginalEntry<ET>
where
    ET: UnitEnum,
{
    Agent(AgentPubKey),
    App(ET),
    PrivateApp(<ET as UnitEnum>::Unit),
    CapClaim,
    CapGrant,
    OutOfScope,
}

fn original_entry<ET>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
    entry: Option<&Entry>,
) -> Result<OriginalEntry<ET>, WasmError>
where
    ET: EntryTypesHelper + UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    WasmError: From<<ET as EntryTypesHelper>::Error>,
{
    match entry_type {
        EntryType::App(AppEntryType {
            zome_id,
            id: entry_def_index,
            visibility,
        }) => {
            let r = match (visibility, entry) {
                (EntryVisibility::Public, None) => {
                    todo!("Make guest error because this is malformed")
                }
                (EntryVisibility::Public, Some(entry)) => {
                    <ET as EntryTypesHelper>::deserialize_from_type(
                        *zome_id,
                        *entry_def_index,
                        entry,
                    )?
                    .map(OriginalEntry::App)
                }
                (EntryVisibility::Private, None) => {
                    get_unit_entry_type::<ET>(*zome_id, *entry_def_index)?
                        .map(OriginalEntry::PrivateApp)
                }
                (EntryVisibility::Private, Some(_)) => {
                    todo!("Make guest error because this is malformed")
                }
            };
            match r {
                Some(o) => Ok(o),
                None => Ok(OriginalEntry::OutOfScope),
            }
        }
        EntryType::AgentPubKey => Ok(OriginalEntry::Agent(entry_hash.clone().into())),
        EntryType::CapClaim => Ok(OriginalEntry::CapClaim),
        EntryType::CapGrant => Ok(OriginalEntry::CapGrant),
    }
}

fn get_unit_entry_type<ET>(
    zome_id: ZomeId,
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
            zome_id,
            zome_type: entry_def_index,
        },
    );
    let unit = match unit {
        Some(unit) => Some(unit),
        None => {
            if entries.dependencies().any(|z| z == zome_id) {
                return Err(wasm_error!(WasmErrorInner::Guest(format!(
                    "Entry type: {:?} is out of range for this zome.",
                    entry_def_index
                ))));
            } else {
                None
                // return Err(wasm_error!(WasmErrorInner::Host(format!(
                //     "Should not be called with the wrong ZomeId {}. This is a holochain bug",
                //     zome_id
                // ))));
            }
        }
    };
    Ok(unit)
}

fn activity_create<ET, LT>(
    entry_type: &EntryType,
    entry_hash: &EntryHash,
) -> Result<OpActivity<<ET as UnitEnum>::Unit, LT>, WasmError>
where
    ET: UnitEnum,
    <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
    LT: LinkTypesHelper,
    WasmError: From<<LT as LinkTypesHelper>::Error>,
{
    match entry_type {
        EntryType::App(AppEntryType {
            zome_id,
            id: entry_def_index,
            visibility,
        }) => {
            let entries = zome_info()?.zome_types.entries;
            let unit = entries.find(
                <ET as UnitEnum>::unit_iter(),
                ScopedEntryDefIndex {
                    zome_id: *zome_id,
                    zome_type: *entry_def_index,
                },
            );
            let unit = match unit {
                Some(unit) => Some(unit),
                None => {
                    if entries.dependencies().any(|z| z == *zome_id) {
                        return Err(wasm_error!(WasmErrorInner::Guest(format!(
                            "Entry type: {:?} is out of range for this zome.",
                            entry_def_index
                        ))));
                    } else {
                        None
                    }
                }
            };
            match visibility {
                EntryVisibility::Public => Ok(OpActivity::CreateEntry {
                    entry_hash: entry_hash.clone(),
                    entry_type: unit,
                }),
                EntryVisibility::Private => Ok(OpActivity::CreatePrivateEntry {
                    entry_hash: entry_hash.clone(),
                    entry_type: unit,
                }),
            }
        }
        EntryType::AgentPubKey => Ok(OpActivity::CreateAgent(entry_hash.clone().into())),
        EntryType::CapClaim => Ok(OpActivity::CreateCapClaim(entry_hash.clone())),
        EntryType::CapGrant => Ok(OpActivity::CreateCapGrant(entry_hash.clone())),
    }
}
