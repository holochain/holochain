//! v2 of [`OpHelper`](crate::op::OpHelper): flattens a v2
//! [`Op`](holochain_integrity_types::dht_v2::Op) into the v2
//! [`FlatOp`](crate::flat_op_v2::FlatOp). Transitional staging module; promoted
//! to replace `op`'s helper in the legacy-deletion phase.

use crate::prelude::*;

/// Conversion from a v2 [`Op`](holochain_integrity_types::dht_v2::Op) to a v2
/// [`FlatOp`](crate::flat_op_v2::FlatOp), for use in the validate callback.
pub trait OpHelper {
    /// Convert without consuming, cloning the required internal data.
    fn flattened<ET, LT>(&self) -> Result<crate::flat_op_v2::FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>;
}

use crate::flat_op_v2;
use crate::op::{
    activity_entry, activity_link_type, get_app_entry_type_for_record_authority,
    get_app_entry_type_for_store_entry_authority, in_scope_link_type, ActivityEntry,
};
use holochain_integrity_types::dht_v2::{self, ActionData};

impl OpHelper for dht_v2::Op {
    fn flattened<ET, LT>(&self) -> Result<flat_op_v2::FlatOp<ET, LT>, WasmError>
    where
        ET: EntryTypesHelper + UnitEnum,
        <ET as UnitEnum>::Unit: Into<ZomeEntryTypesKey>,
        LT: LinkTypesHelper,
        WasmError: From<<ET as EntryTypesHelper>::Error>,
        WasmError: From<<LT as LinkTypesHelper>::Error>,
    {
        match self {
            dht_v2::Op::StoreRecord(dht_v2::StoreRecord { record }) => {
                let a = record.action();
                let r = match &a.data {
                    ActionData::Dna(d) => flat_op_v2::OpRecord::Dna {
                        dna_hash: d.dna_hash.clone(),
                        action: a.clone(),
                    },
                    ActionData::AgentValidationPkg(d) => flat_op_v2::OpRecord::AgentValidationPkg {
                        membrane_proof: d.membrane_proof.clone(),
                        action: a.clone(),
                    },
                    ActionData::InitZomesComplete(_) => {
                        flat_op_v2::OpRecord::InitZomesComplete { action: a.clone() }
                    }
                    ActionData::OpenChain(_) => flat_op_v2::OpRecord::open_chain(a.clone()),
                    ActionData::CloseChain(_) => flat_op_v2::OpRecord::close_chain(a.clone()),
                    ActionData::CreateLink(d) => {
                        let link_type = in_scope_link_type(d.zome_index, d.link_type)?;
                        flat_op_v2::OpRecord::CreateLink {
                            base_address: d.base_address.clone(),
                            target_address: d.target_address.clone(),
                            tag: d.tag.clone(),
                            link_type,
                            action: a.clone(),
                        }
                    }
                    ActionData::DeleteLink(d) => flat_op_v2::OpRecord::DeleteLink {
                        original_action_hash: d.link_add_address.clone(),
                        base_address: d.base_address.clone(),
                        action: a.clone(),
                    },
                    ActionData::Create(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op_v2::OpRecord::CreateAgent {
                            agent: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => {
                            match get_app_entry_type_for_record_authority::<ET>(
                                entry_def,
                                record.entry.as_option(),
                            )? {
                                UnitEnumEither::Enum(app_entry) => {
                                    flat_op_v2::OpRecord::CreateEntry {
                                        app_entry,
                                        action: a.clone(),
                                    }
                                }
                                UnitEnumEither::Unit(app_entry_type) => {
                                    flat_op_v2::OpRecord::CreatePrivateEntry {
                                        app_entry_type,
                                        action: a.clone(),
                                    }
                                }
                            }
                        }
                        EntryType::CapClaim => {
                            flat_op_v2::OpRecord::CreateCapClaim { action: a.clone() }
                        }
                        EntryType::CapGrant => {
                            flat_op_v2::OpRecord::CreateCapGrant { action: a.clone() }
                        }
                    },
                    ActionData::Update(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op_v2::OpRecord::UpdateAgent {
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
                                UnitEnumEither::Enum(app_entry) => {
                                    flat_op_v2::OpRecord::UpdateEntry {
                                        original_action_hash: d.original_action_address.clone(),
                                        original_entry_hash: d.original_entry_address.clone(),
                                        app_entry,
                                        action: a.clone(),
                                    }
                                }
                                UnitEnumEither::Unit(app_entry_type) => {
                                    flat_op_v2::OpRecord::UpdatePrivateEntry {
                                        original_action_hash: d.original_action_address.clone(),
                                        original_entry_hash: d.original_entry_address.clone(),
                                        app_entry_type,
                                        action: a.clone(),
                                    }
                                }
                            }
                        }
                        EntryType::CapClaim => flat_op_v2::OpRecord::UpdateCapClaim {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            action: a.clone(),
                        },
                        EntryType::CapGrant => flat_op_v2::OpRecord::UpdateCapGrant {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            action: a.clone(),
                        },
                    },
                    ActionData::Delete(d) => flat_op_v2::OpRecord::DeleteEntry {
                        original_action_hash: d.deletes_address.clone(),
                        original_entry_hash: d.deletes_entry_address.clone(),
                        action: a.clone(),
                    },
                };
                Ok(flat_op_v2::FlatOp::StoreRecord(r))
            }
            dht_v2::Op::StoreEntry(dht_v2::StoreEntry { action, entry }) => {
                let a = &action.hashed.content;
                let r = match &a.data {
                    ActionData::Create(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op_v2::OpEntry::CreateAgent {
                            agent: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => flat_op_v2::OpEntry::CreateEntry {
                            app_entry: get_app_entry_type_for_store_entry_authority(entry_def, entry)?,
                            action: a.clone(),
                        },
                        EntryType::CapClaim => flat_op_v2::OpEntry::CreateCapClaim {
                            entry: cap_claim_entry(entry)?,
                            action: a.clone(),
                        },
                        EntryType::CapGrant => flat_op_v2::OpEntry::CreateCapGrant {
                            entry: cap_grant_entry(entry)?,
                            action: a.clone(),
                        },
                    },
                    ActionData::Update(d) => match &d.entry_type {
                        EntryType::AgentPubKey => flat_op_v2::OpEntry::UpdateAgent {
                            original_key: d.original_entry_address.clone().into(),
                            original_action_hash: d.original_action_address.clone(),
                            new_key: d.entry_hash.clone().into(),
                            action: a.clone(),
                        },
                        EntryType::App(entry_def) => flat_op_v2::OpEntry::UpdateEntry {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            app_entry: get_app_entry_type_for_store_entry_authority(entry_def, entry)?,
                            action: a.clone(),
                        },
                        EntryType::CapClaim => flat_op_v2::OpEntry::UpdateCapClaim {
                            original_action_hash: d.original_action_address.clone(),
                            original_entry_hash: d.original_entry_address.clone(),
                            entry: cap_claim_entry(entry)?,
                            action: a.clone(),
                        },
                        EntryType::CapGrant => flat_op_v2::OpEntry::UpdateCapGrant {
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
                Ok(flat_op_v2::FlatOp::StoreEntry(r))
            }
            dht_v2::Op::RegisterUpdate(dht_v2::RegisterUpdate { update, new_entry }) => {
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
                    EntryType::AgentPubKey => flat_op_v2::OpUpdate::Agent {
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
                            UnitEnumEither::Enum(new) => flat_op_v2::OpUpdate::Entry {
                                app_entry: new,
                                action: a.clone(),
                            },
                            UnitEnumEither::Unit(new) => flat_op_v2::OpUpdate::PrivateEntry {
                                original_action_hash: d.original_action_address.clone(),
                                app_entry_type: new,
                                action: a.clone(),
                            },
                        }
                    }
                    EntryType::CapClaim => flat_op_v2::OpUpdate::CapClaim {
                        original_action_hash: d.original_action_address.clone(),
                        action: a.clone(),
                    },
                    EntryType::CapGrant => flat_op_v2::OpUpdate::CapGrant {
                        original_action_hash: d.original_action_address.clone(),
                        action: a.clone(),
                    },
                };
                Ok(flat_op_v2::FlatOp::RegisterUpdate(r))
            }
            dht_v2::Op::RegisterAgentActivity(dht_v2::RegisterAgentActivity { action, .. }) => {
                let a = &action.hashed.content;
                let r = match &a.data {
                    ActionData::Dna(d) => flat_op_v2::OpActivity::Dna {
                        dna_hash: d.dna_hash.clone(),
                        action: a.clone(),
                    },
                    ActionData::AgentValidationPkg(d) => {
                        flat_op_v2::OpActivity::AgentValidationPkg {
                            membrane_proof: d.membrane_proof.clone(),
                            action: a.clone(),
                        }
                    }
                    ActionData::InitZomesComplete(_) => {
                        flat_op_v2::OpActivity::InitZomesComplete { action: a.clone() }
                    }
                    ActionData::OpenChain(_) => flat_op_v2::OpActivity::open_chain(a.clone()),
                    ActionData::CloseChain(_) => flat_op_v2::OpActivity::close_chain(a.clone()),
                    ActionData::CreateLink(d) => {
                        let link_type = activity_link_type(d.zome_index, d.link_type)?;
                        flat_op_v2::OpActivity::CreateLink {
                            base_address: d.base_address.clone(),
                            target_address: d.target_address.clone(),
                            tag: d.tag.clone(),
                            link_type,
                            action: a.clone(),
                        }
                    }
                    ActionData::DeleteLink(d) => flat_op_v2::OpActivity::DeleteLink {
                        original_action_hash: d.link_add_address.clone(),
                        base_address: d.base_address.clone(),
                        action: a.clone(),
                    },
                    ActionData::Create(d) => {
                        match activity_entry::<ET>(&d.entry_type, &d.entry_hash)? {
                            ActivityEntry::App { entry_type, .. } => {
                                flat_op_v2::OpActivity::CreateEntry {
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                flat_op_v2::OpActivity::CreatePrivateEntry {
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::Agent(agent) => flat_op_v2::OpActivity::CreateAgent {
                                agent,
                                action: a.clone(),
                            },
                            ActivityEntry::CapClaim(_) => {
                                flat_op_v2::OpActivity::CreateCapClaim { action: a.clone() }
                            }
                            ActivityEntry::CapGrant(_) => {
                                flat_op_v2::OpActivity::CreateCapGrant { action: a.clone() }
                            }
                        }
                    }
                    ActionData::Update(d) => {
                        match activity_entry::<ET>(&d.entry_type, &d.entry_hash)? {
                            ActivityEntry::App { entry_type, .. } => {
                                flat_op_v2::OpActivity::UpdateEntry {
                                    original_action_hash: d.original_action_address.clone(),
                                    original_entry_hash: d.original_entry_address.clone(),
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::PrivateApp { entry_type, .. } => {
                                flat_op_v2::OpActivity::UpdatePrivateEntry {
                                    original_action_hash: d.original_action_address.clone(),
                                    original_entry_hash: d.original_entry_address.clone(),
                                    app_entry_type: entry_type,
                                    action: a.clone(),
                                }
                            }
                            ActivityEntry::Agent(new_key) => flat_op_v2::OpActivity::UpdateAgent {
                                original_action_hash: d.original_action_address.clone(),
                                original_key: d.original_entry_address.clone().into(),
                                new_key,
                                action: a.clone(),
                            },
                            ActivityEntry::CapClaim(_) => flat_op_v2::OpActivity::UpdateCapClaim {
                                original_action_hash: d.original_action_address.clone(),
                                original_entry_hash: d.original_entry_address.clone(),
                                action: a.clone(),
                            },
                            ActivityEntry::CapGrant(_) => flat_op_v2::OpActivity::UpdateCapGrant {
                                original_action_hash: d.original_action_address.clone(),
                                original_entry_hash: d.original_entry_address.clone(),
                                action: a.clone(),
                            },
                        }
                    }
                    ActionData::Delete(d) => flat_op_v2::OpActivity::DeleteEntry {
                        original_action_hash: d.deletes_address.clone(),
                        original_entry_hash: d.deletes_entry_address.clone(),
                        action: a.clone(),
                    },
                };
                Ok(flat_op_v2::FlatOp::RegisterAgentActivity(r))
            }
            dht_v2::Op::RegisterCreateLink(dht_v2::RegisterCreateLink { create_link }) => {
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
                Ok(flat_op_v2::FlatOp::RegisterCreateLink {
                    base_address: d.base_address.clone(),
                    target_address: d.target_address.clone(),
                    tag: d.tag.clone(),
                    link_type,
                    action: a.clone(),
                })
            }
            dht_v2::Op::RegisterDeleteLink(dht_v2::RegisterDeleteLink {
                delete_link,
                create_link,
            }) => {
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
                Ok(flat_op_v2::FlatOp::RegisterDeleteLink {
                    original_action: create_link.clone(),
                    base_address: d.base_address.clone(),
                    target_address: d.target_address.clone(),
                    tag: d.tag.clone(),
                    link_type,
                    action: delete_link.hashed.content.clone(),
                })
            }
            dht_v2::Op::RegisterDelete(dht_v2::RegisterDelete { delete }) => {
                Ok(flat_op_v2::FlatOp::RegisterDelete(flat_op_v2::OpDelete {
                    action: delete.hashed.content.clone(),
                }))
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
