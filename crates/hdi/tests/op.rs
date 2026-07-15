//! Coverage for the [`OpHelper::flattened`] conversion (`hdi::op`),
//! the `Op` -> `FlatOp` path used by the `validate` callback.
//!
//! `get_unit_entry_type`, `deny_other_zome`,
//! `get_app_entry_type_for_record_authority`, and
//! `get_app_entry_type_for_store_entry_authority` are private to
//! `hdi::op`, so their error branches are exercised the same way
//! production code reaches them: through the public [`OpHelper::flattened`]
//! method.
//!
//! Three groups of tests:
//! - `op_errors`: every error branch of these helpers, reached through every
//!   `Op` variant that calls them.
//! - the individual round-trip tests: a representative `Op` -> `FlatOp`
//!   subset covering every `Op`/`FlatOp` variant, private entries, and link
//!   create/delete.
//! - `op_match_sanity`: a compile-time exhaustiveness guard over the
//!   `FlatOp`/`OpRecord`/`OpLink` match arms.

use hdi::prelude::*;
use hdi::test_utils::set_zome_types;
use hdi::test_utils::short_hand::*;
use std::sync::Arc;
use test_case::test_case;

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct A;
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct B;
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq)]
pub struct C;

/// Same wire shape as a struct entry but with fields, so serializing it and
/// deserializing the result as [`A`] (a unit struct) fails. Used to exercise
/// the deserialize-failure branch of the entry-type helpers.
#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Default)]
pub struct D {
    a: (),
    b: (),
}

#[hdk_entry_types(skip_hdk_extern = true)]
#[unit_enum(UnitEntryTypes)]
#[derive(Clone, PartialEq, Eq)]
pub enum EntryTypes {
    A(A),
    #[entry_type(visibility = "private")]
    B(B),
    C(C),
}

#[hdk_link_types(skip_no_mangle = true)]
pub enum LinkTypes {
    A,
    B,
    C,
}

/// Registers zome 0 as a dependency with 3 entry types and 3 link types in
/// scope, matching [`EntryTypes`]/[`LinkTypes`] above. Zome 100 is never
/// registered, standing in for a zome that this zome does not depend on.
fn types() {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
}

// -- action/op builders ------------------------------------------------------

fn header() -> ActionHeader {
    ActionHeader {
        author: ak(0),
        timestamp: Timestamp::from_micros(0),
        action_seq: 1,
        prev_action: Some(ah(0)),
    }
}

fn action(data: ActionData) -> Action {
    Action {
        header: header(),
        data,
    }
}

fn signed(data: ActionData) -> SignedHashed<Action> {
    SignedHashed {
        hashed: HoloHashed {
            content: action(data),
            hash: ah(0),
        },
        signature: Signature([0u8; 64]),
    }
}

fn app_entry_def(zome_index: u8, entry_index: u8, visibility: EntryVisibility) -> AppEntryDef {
    AppEntryDef {
        entry_index: entry_index.into(),
        zome_index: zome_index.into(),
        visibility,
    }
}

fn create_data(entry_type: EntryType) -> ActionData {
    ActionData::Create(CreateData {
        entry_type,
        entry_hash: eh(0),
    })
}

fn create_app_data(zome_index: u8, entry_index: u8, visibility: EntryVisibility) -> ActionData {
    create_data(EntryType::App(app_entry_def(
        zome_index,
        entry_index,
        visibility,
    )))
}

fn update_data(entry_type: EntryType) -> ActionData {
    ActionData::Update(UpdateData {
        original_action_address: ah(1),
        original_entry_address: eh(1),
        entry_type,
        entry_hash: eh(0),
    })
}

fn update_app_data(zome_index: u8, entry_index: u8, visibility: EntryVisibility) -> ActionData {
    update_data(EntryType::App(app_entry_def(
        zome_index,
        entry_index,
        visibility,
    )))
}

fn create_link_data(zome_index: u8, link_type: u8) -> ActionData {
    ActionData::CreateLink(CreateLinkData {
        base_address: eh(0).into(),
        target_address: eh(1).into(),
        zome_index: zome_index.into(),
        link_type: link_type.into(),
        tag: ().into(),
    })
}

fn delete_link_data() -> ActionData {
    ActionData::DeleteLink(DeleteLinkData {
        base_address: eh(0).into(),
        link_add_address: ah(2),
    })
}

fn delete_data() -> ActionData {
    ActionData::Delete(DeleteData {
        deletes_address: ah(2),
        deletes_entry_address: eh(1),
    })
}

fn dna_data(dna_hash: DnaHash) -> ActionData {
    ActionData::Dna(DnaData { dna_hash })
}

fn avp_data(membrane_proof: Option<MembraneProof>) -> ActionData {
    ActionData::AgentValidationPkg(AgentValidationPkgData { membrane_proof })
}

fn izc_data() -> ActionData {
    ActionData::InitZomesComplete(InitZomesCompleteData {})
}

fn open_chain_data(prev_target: MigrationTarget, close_hash: ActionHash) -> ActionData {
    ActionData::OpenChain(OpenChainData {
        prev_target,
        close_hash,
    })
}

fn close_chain_data(new_target: Option<MigrationTarget>) -> ActionData {
    ActionData::CloseChain(CloseChainData { new_target })
}

fn membrane_proof() -> MembraneProof {
    Arc::new(SerializedBytes::default())
}

fn cap_claim() -> CapClaim {
    CapClaim::new("tag".into(), ak(9), [1u8; CAP_SECRET_BYTES].into())
}

fn cap_grant() -> ZomeCallCapGrant {
    ZomeCallCapGrant::new("tag".into(), ().into(), GrantedFunctions::All)
}

fn store_record(data: ActionData, entry: RecordEntry) -> Op {
    Op::CreateRecord(CreateRecord {
        record: Record::new(signed(data), entry),
    })
}

fn store_entry(data: ActionData, entry: Entry) -> Op {
    Op::CreateEntry(CreateEntry {
        action: signed(data),
        entry,
    })
}

fn register_update(data: ActionData, new_entry: Option<Entry>) -> Op {
    Op::Update(Update {
        update: signed(data),
        new_entry,
    })
}

fn register_delete(data: ActionData) -> Op {
    Op::Delete(Delete {
        delete: signed(data),
    })
}

fn register_agent_activity(data: ActionData) -> Op {
    Op::AgentActivity(AgentActivity {
        action: signed(data),
        cached_entry: None,
    })
}

fn register_create_link(zome_index: u8, link_type: u8) -> Op {
    Op::CreateLink(CreateLink {
        create_link: signed(create_link_data(zome_index, link_type)),
    })
}

fn register_delete_link(create_zome_index: u8, create_link_type: u8) -> Op {
    Op::DeleteLink(DeleteLink {
        delete_link: signed(delete_link_data()),
        create_link: action(create_link_data(create_zome_index, create_link_type)),
    })
}

// -- error branches -----------------------------------------------------------
//
// Every case below exercises a failure branch of `get_unit_entry_type`,
// `deny_other_zome`, `get_app_entry_type_for_record_authority`, or
// `get_app_entry_type_for_store_entry_authority` (all private to
// `hdi::op`), plus the `Op`-variant/`ActionData`-shape mismatch guards:
// every `Op` variant carries the same `Action` type, so `flattened` checks
// at runtime that `action.data` is the variant each `Op` case requires.

// AgentActivity
#[test_case(register_agent_activity(create_app_data(0, 100, EntryVisibility::Public))
    => matches WasmErrorInner::Guest(_) ; "AgentActivity: create entry type index out of range")]
#[test_case(register_agent_activity(create_link_data(0, 100))
    => matches WasmErrorInner::Guest(_) ; "AgentActivity: create link type out of range")]
// CreateRecord
#[test_case(store_record(create_app_data(0, 100, EntryVisibility::Private), RecordEntry::Hidden)
    => matches WasmErrorInner::Guest(_) ; "CreateRecord: private entry type index out of range")]
#[test_case(store_record(create_app_data(100, 0, EntryVisibility::Private), RecordEntry::Hidden)
    => matches WasmErrorInner::Host(_) ; "CreateRecord: private entry zome out of scope")]
#[test_case(store_record(create_app_data(0, 0, EntryVisibility::Public), RecordEntry::Present(e(D::default())))
    => WasmErrorInner::Serialize(SerializedBytesError::Deserialize("invalid type: map, expected unit struct A".to_string()))
    ; "CreateRecord: entry fails to deserialize as the target app entry type")]
#[test_case(store_record(create_app_data(0, 100, EntryVisibility::Public), RecordEntry::Present(e(A {})))
    => matches WasmErrorInner::Guest(_) ; "CreateRecord: public entry type index out of range")]
#[test_case(store_record(create_app_data(100, 0, EntryVisibility::Public), RecordEntry::Present(e(A {})))
    => matches WasmErrorInner::Host(_) ; "CreateRecord: public entry zome out of scope")]
#[test_case(store_record(create_app_data(0, 0, EntryVisibility::Private), RecordEntry::Present(e(A {})))
    => matches WasmErrorInner::Guest(_) ; "CreateRecord: private entry type but entry is present")]
#[test_case(store_record(create_app_data(0, 0, EntryVisibility::Public), RecordEntry::NA)
    => matches WasmErrorInner::Guest(_) ; "CreateRecord: public entry type but entry is absent")]
#[test_case(store_record(create_link_data(0, 100), RecordEntry::NA)
    => matches WasmErrorInner::Guest(_) ; "CreateRecord: link type out of range")]
#[test_case(store_record(create_link_data(100, 0), RecordEntry::NA)
    => matches WasmErrorInner::Host(_) ; "CreateRecord: link zome out of scope")]
// CreateEntry
#[test_case(store_entry(create_app_data(0, 100, EntryVisibility::Public), e(A {}))
    => matches WasmErrorInner::Guest(_) ; "CreateEntry: entry type index out of range")]
#[test_case(store_entry(create_app_data(100, 0, EntryVisibility::Public), e(A {}))
    => matches WasmErrorInner::Host(_) ; "CreateEntry: entry zome out of scope")]
#[test_case(store_entry(create_app_data(0, 0, EntryVisibility::Public), e(D::default()))
    => WasmErrorInner::Serialize(SerializedBytesError::Deserialize("invalid type: map, expected unit struct A".to_string()))
    ; "CreateEntry: entry fails to deserialize as the target app entry type")]
#[test_case(store_entry(create_data(EntryType::CapClaim), e(A {}))
    => matches WasmErrorInner::Guest(_) ; "CreateEntry: entry does not match CapClaim")]
#[test_case(store_entry(create_data(EntryType::CapGrant), e(A {}))
    => matches WasmErrorInner::Guest(_) ; "CreateEntry: entry does not match CapGrant")]
#[test_case(store_entry(delete_data(), e(A {}))
    => matches WasmErrorInner::Guest(_) ; "CreateEntry: action data is not an entry-creation action")]
// Update
#[test_case(register_update(update_app_data(0, 0, EntryVisibility::Public), Some(e(D::default())))
    => matches WasmErrorInner::Serialize(_) ; "Update: new entry fails to deserialize")]
#[test_case(register_update(update_app_data(0, 0, EntryVisibility::Public), None)
    => matches WasmErrorInner::Guest(_) ; "Update: new entry is missing")]
#[test_case(register_update(update_app_data(0, 0, EntryVisibility::Private), Some(e(A {})))
    => matches WasmErrorInner::Guest(_) ; "Update: new entry is private but also present")]
#[test_case(register_update(update_app_data(0, 100, EntryVisibility::Public), Some(e(A {})))
    => matches WasmErrorInner::Guest(_) ; "Update: entry type index out of range")]
#[test_case(register_update(update_app_data(100, 0, EntryVisibility::Public), Some(e(A {})))
    => matches WasmErrorInner::Host(_) ; "Update: zome id out of range")]
#[test_case(register_update(create_app_data(0, 0, EntryVisibility::Public), Some(e(A {})))
    => matches WasmErrorInner::Guest(_) ; "Update: action data is not an Update action")]
// CreateLink / DeleteLink
#[test_case(register_create_link(0, 100)
    => matches WasmErrorInner::Guest(_) ; "CreateLink: link type out of range")]
#[test_case(register_create_link(100, 0)
    => matches WasmErrorInner::Host(_) ; "CreateLink: zome id out of range")]
#[test_case(Op::CreateLink(CreateLink { create_link: signed(delete_data()) })
    => matches WasmErrorInner::Guest(_) ; "CreateLink: action data is not a CreateLink action")]
#[test_case(register_delete_link(0, 100)
    => matches WasmErrorInner::Guest(_) ; "DeleteLink: original link type out of range")]
#[test_case(register_delete_link(100, 0)
    => matches WasmErrorInner::Host(_) ; "DeleteLink: original zome id out of range")]
#[test_case(Op::DeleteLink(DeleteLink { delete_link: signed(delete_data()), create_link: action(create_link_data(0, 0)) })
    => matches WasmErrorInner::Guest(_) ; "DeleteLink: delete action is not a DeleteLink action")]
#[test_case(Op::DeleteLink(DeleteLink { delete_link: signed(delete_link_data()), create_link: action(delete_data()) })
    => matches WasmErrorInner::Guest(_) ; "DeleteLink: original action is not a CreateLink action")]
// Delete
#[test_case(register_delete(create_app_data(0, 0, EntryVisibility::Public))
    => matches WasmErrorInner::Guest(_) ; "Delete: action data is not a Delete action")]
fn op_errors(op: Op) -> WasmErrorInner {
    types();
    op.flattened::<EntryTypes, LinkTypes>().unwrap_err().error
}

// -- round-trip: Op -> FlatOp, one representative case per Op/FlatOp variant,
// plus private entries and link create/delete --------------------------------

#[test]
fn store_record_create_public_entry_flattens_to_create_entry() {
    types();
    let op = store_record(
        create_app_data(0, 0, EntryVisibility::Public),
        RecordEntry::Present(e(A {})),
    );
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
fn store_record_create_private_entry_flattens_to_create_private_entry() {
    types();
    let op = store_record(
        create_app_data(0, 1, EntryVisibility::Private),
        RecordEntry::Hidden,
    );
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::CreateRecord(OpRecord::CreatePrivateEntry {
            app_entry_type: UnitEntryTypes::B,
            ..
        })
    ));
}

#[test]
fn store_record_update_entry_flattens_to_update_entry() {
    types();
    let op = store_record(
        update_app_data(0, 2, EntryVisibility::Public),
        RecordEntry::Present(e(C {})),
    );
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::CreateRecord(OpRecord::UpdateEntry {
            app_entry: EntryTypes::C(C {}),
            ..
        })
    ));
}

#[test]
fn store_record_delete_entry_flattens_to_delete_entry() {
    types();
    let op = store_record(delete_data(), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::CreateRecord(OpRecord::DeleteEntry { .. })
    ));
}

#[test]
fn store_record_create_link_flattens_to_create_link() {
    types();
    let op = store_record(create_link_data(0, 0), RecordEntry::NA);
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
fn store_record_delete_link_flattens_to_delete_link() {
    types();
    let op = store_record(delete_link_data(), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::CreateRecord(OpRecord::DeleteLink { .. })
    ));
}

#[test]
fn store_record_dna_flattens_to_dna() {
    types();
    let hash = dh(3);
    let op = store_record(dna_data(hash.clone()), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    match flat {
        FlatOp::CreateRecord(OpRecord::Dna { dna_hash, .. }) => assert_eq!(dna_hash, hash),
        _ => panic!("expected Dna"),
    }
}

#[test]
fn store_record_open_chain_flattens_to_open_chain() {
    types();
    let target = MigrationTarget::Dna(dh(1));
    let op = store_record(open_chain_data(target.clone(), ah(9)), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    match flat {
        FlatOp::CreateRecord(OpRecord::OpenChain {
            previous_target,
            close_hash,
            ..
        }) => {
            assert_eq!(previous_target, target);
            assert_eq!(close_hash, ah(9));
        }
        _ => panic!("expected OpenChain"),
    }
}

#[test]
fn store_record_close_chain_flattens_to_close_chain() {
    types();
    let target = MigrationTarget::Dna(dh(2));
    let op = store_record(close_chain_data(Some(target.clone())), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    match flat {
        FlatOp::CreateRecord(OpRecord::CloseChain { new_target, .. }) => {
            assert_eq!(new_target, Some(target));
        }
        _ => panic!("expected CloseChain"),
    }
}

#[test]
fn store_record_agent_validation_pkg_flattens_to_agent_validation_pkg() {
    types();
    let proof = membrane_proof();
    let op = store_record(avp_data(Some(proof.clone())), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    match flat {
        FlatOp::CreateRecord(OpRecord::AgentValidationPkg { membrane_proof, .. }) => {
            assert_eq!(membrane_proof, Some(proof));
        }
        _ => panic!("expected AgentValidationPkg"),
    }
}

#[test]
fn store_record_init_zomes_complete_flattens_to_init_zomes_complete() {
    types();
    let op = store_record(izc_data(), RecordEntry::NA);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::CreateRecord(OpRecord::InitZomesComplete { .. })
    ));
}

#[test]
fn store_entry_create_entry_flattens_to_create_entry() {
    types();
    let op = store_entry(create_app_data(0, 0, EntryVisibility::Public), e(A {}));
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
fn store_entry_create_cap_claim_flattens_to_create_cap_claim() {
    types();
    let claim = cap_claim();
    let op = store_entry(
        create_data(EntryType::CapClaim),
        Entry::CapClaim(claim.clone()),
    );
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    match flat {
        FlatOp::CreateEntry(OpEntry::CreateCapClaim { entry, .. }) => assert_eq!(entry, claim),
        _ => panic!("expected CreateCapClaim"),
    }
}

#[test]
fn store_entry_create_cap_grant_flattens_to_create_cap_grant() {
    types();
    let grant = cap_grant();
    let op = store_entry(
        create_data(EntryType::CapGrant),
        Entry::CapGrant(grant.clone()),
    );
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    match flat {
        FlatOp::CreateEntry(OpEntry::CreateCapGrant { entry, .. }) => assert_eq!(entry, grant),
        _ => panic!("expected CreateCapGrant"),
    }
}

#[test]
fn register_update_entry_flattens_to_update_entry() {
    types();
    let op = register_update(
        update_app_data(0, 0, EntryVisibility::Public),
        Some(e(A {})),
    );
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::Update(OpUpdate::Entry {
            app_entry: EntryTypes::A(A {}),
            ..
        })
    ));
}

#[test]
fn register_update_private_entry_flattens_to_update_private_entry() {
    types();
    let op = register_update(update_app_data(0, 1, EntryVisibility::Private), None);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::Update(OpUpdate::PrivateEntry {
            app_entry_type: UnitEntryTypes::B,
            ..
        })
    ));
}

#[test]
fn register_update_cap_claim_flattens_to_cap_claim() {
    types();
    let op = register_update(update_data(EntryType::CapClaim), None);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(flat, FlatOp::Update(OpUpdate::CapClaim { .. })));
}

#[test]
fn register_delete_flattens_to_register_delete() {
    types();
    let op = register_delete(delete_data());
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(flat, FlatOp::Delete(_)));
}

#[test]
fn register_agent_activity_create_private_entry_flattens_with_unit_type() {
    types();
    let op = register_agent_activity(create_app_data(0, 1, EntryVisibility::Private));
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::AgentActivity(OpActivity::CreatePrivateEntry {
            app_entry_type: Some(UnitEntryTypes::B),
            ..
        })
    ));
}

#[test]
fn register_agent_activity_create_entry_out_of_scope_zome_flattens_with_none_type() {
    types();
    let op = register_agent_activity(create_app_data(100, 0, EntryVisibility::Public));
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::AgentActivity(OpActivity::CreateEntry {
            app_entry_type: None,
            ..
        })
    ));
}

#[test]
fn register_agent_activity_create_link_flattens_with_link_type() {
    types();
    let op = register_agent_activity(create_link_data(0, 1));
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::AgentActivity(OpActivity::CreateLink {
            link_type: Some(LinkTypes::B),
            ..
        })
    ));
}

#[test]
fn register_agent_activity_create_link_out_of_scope_zome_flattens_with_none_type() {
    types();
    let op = register_agent_activity(create_link_data(100, 0));
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::AgentActivity(OpActivity::CreateLink {
            link_type: None,
            ..
        })
    ));
}

#[test]
fn register_create_link_flattens_to_register_link_create() {
    types();
    let op = register_create_link(0, 2);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::Link(OpLink::CreateLink {
            link_type: LinkTypes::C,
            ..
        })
    ));
}

#[test]
fn register_delete_link_flattens_to_register_link_delete() {
    types();
    let op = register_delete_link(0, 0);
    let flat: FlatOp<EntryTypes, LinkTypes> = op.flattened().unwrap();
    assert!(matches!(
        flat,
        FlatOp::Link(OpLink::DeleteLink {
            link_type: LinkTypes::A,
            ..
        })
    ));
}

/// Compile-time exhaustiveness guard: matches every `FlatOp`/`OpRecord`/
/// `OpLink` arm with no wildcard, so adding a variant to any of these types
/// breaks this test until it is updated to handle the new arm.
#[test]
fn op_match_sanity() {
    types();
    let op = store_record(
        create_app_data(0, 0, EntryVisibility::Public),
        RecordEntry::Present(e(A {})),
    );
    match op.flattened::<EntryTypes, LinkTypes>().unwrap() {
        FlatOp::CreateRecord(r) => match r {
            OpRecord::CreateEntry {
                app_entry: EntryTypes::A(_),
                ..
            } => (),
            OpRecord::CreateEntry {
                app_entry: EntryTypes::B(_),
                ..
            } => unreachable!(),
            OpRecord::CreateEntry {
                app_entry: EntryTypes::C(_),
                ..
            } => (),
            OpRecord::CreatePrivateEntry {
                app_entry_type: UnitEntryTypes::B,
                ..
            } => (),
            OpRecord::CreatePrivateEntry { .. } => unreachable!(),
            OpRecord::CreateAgent { .. } => (),
            OpRecord::CreateCapClaim { .. } => (),
            OpRecord::CreateCapGrant { .. } => (),
            OpRecord::UpdateEntry {
                app_entry: EntryTypes::A(_),
                ..
            } => (),
            OpRecord::UpdateEntry {
                app_entry: EntryTypes::B(_),
                ..
            } => unreachable!(),
            OpRecord::UpdateEntry {
                app_entry: EntryTypes::C(_),
                ..
            } => (),
            OpRecord::UpdatePrivateEntry {
                app_entry_type: UnitEntryTypes::B,
                ..
            } => (),
            OpRecord::UpdatePrivateEntry { .. } => unreachable!(),
            OpRecord::UpdateAgent { .. } => (),
            OpRecord::UpdateCapClaim { .. } => (),
            OpRecord::UpdateCapGrant { .. } => (),
            OpRecord::DeleteEntry { .. } => (),
            OpRecord::CreateLink {
                link_type: LinkTypes::A,
                ..
            } => (),
            OpRecord::CreateLink {
                link_type: LinkTypes::B,
                ..
            } => (),
            OpRecord::CreateLink {
                link_type: LinkTypes::C,
                ..
            } => (),
            OpRecord::DeleteLink { .. } => (),
            OpRecord::Dna { .. } => (),
            OpRecord::OpenChain { .. } => (),
            OpRecord::CloseChain { .. } => (),
            OpRecord::AgentValidationPkg { .. } => (),
            OpRecord::InitZomesComplete { .. } => (),
        },
        FlatOp::CreateEntry(_) => (),
        FlatOp::AgentActivity(_) => (),
        FlatOp::Link(link) => match link {
            OpLink::CreateLink { link_type, .. } => match link_type {
                LinkTypes::A => (),
                LinkTypes::B => (),
                LinkTypes::C => (),
            },
            OpLink::DeleteLink { link_type, .. } => match link_type {
                LinkTypes::A => (),
                LinkTypes::B => (),
                LinkTypes::C => (),
            },
        },
        FlatOp::Update(_) => (),
        FlatOp::Delete(_) => (),
    }
}
