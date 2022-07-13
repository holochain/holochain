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
                    Action::Delete(_) => todo!(),
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
                original_entry,
                ..
            }) => {
                let Update {
                    original_action_address: original_action_hash,
                    original_entry_address: original_entry_hash,
                    entry_type,
                    entry_hash,
                    ..
                } = &update.hashed.content;
                let r = match store_entry_create::<ET>(entry_type, entry_hash, new_entry)? {
                    OpEntry::CreateEntry {
                        entry_hash,
                        entry_type: new_entry_type,
                    } => match store_entry_create::<ET>(entry_type, &entry_hash, original_entry)? {
                        OpEntry::CreateEntry {
                            entry_type: original_entry_type,
                            ..
                        } => OpUpdate::Entry {
                            entry_hash,
                            original_action_hash: original_action_hash.clone(),
                            original_entry_hash: original_entry_hash.clone(),
                            new_entry_type,
                            original_entry_type,
                        },
                        _ => todo!("guest error because types don't match"),
                    },
                    OpEntry::CreateAgent(new_key) => OpUpdate::Agent {
                        original_key: original_entry_hash.clone().into(),
                        original_action_hash: original_action_hash.clone(),
                        new_key,
                    },
                    _ => unreachable!("This record is never created in this arm"),
                };
                Ok(OpType::RegisterUpdate(r))
            }
            Op::RegisterDelete(_) => todo!(),
            Op::RegisterAgentActivity(_) => todo!(),
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
                let entries = zome_info()?.zome_types.entries;
                let unit = entries.find(
                    <ET as UnitEnum>::unit_iter(),
                    ScopedEntryDefIndex {
                        zome_id: *zome_id,
                        zome_type: *entry_def_index,
                    },
                );
                let unit = match unit {
                    Some(unit) => unit,
                    None => {
                        if entries.dependencies().any(|z| z == *zome_id) {
                            return Err(wasm_error!(WasmErrorInner::Guest(format!(
                                "Entry type: {:?} is out of range for this zome.",
                                entry_def_index
                            ))));
                        } else {
                            return Err(wasm_error!(WasmErrorInner::Host(format!(
                                "StoreRecord should not be called with the wrong ZomeId {}. This is a holochain bug",
                                zome_id
                            ))));
                        }
                    }
                };
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
