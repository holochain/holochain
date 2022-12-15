use super::set_zome_types;
use arbitrary::Arbitrary;
use arbitrary::Unstructured;
use hdi::prelude::*;
use hdi::test_utils::short_hand::*;
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

#[hdk_entry_helper]
#[derive(Clone, PartialEq, Eq, Default)]
pub struct D {
    a: (),
    b: (),
}

#[hdk_entry_defs(skip_hdk_extern = true)]
#[unit_enum(UnitEntryTypes)]
#[derive(Clone, PartialEq, Eq)]
pub enum EntryTypes {
    A(A),
    #[entry_def(visibility = "private")]
    B(B),
    C(C),
}
#[hdk_link_types(skip_no_mangle = true)]
pub enum LinkTypes {
    A,
    B,
    C,
}

// Register Agent Activity
#[test_case(r_activity(create_entry(0, 100)) => matches WasmErrorInner::Guest(_))]
#[test_case(r_activity(create_link(0, 100)) => matches WasmErrorInner::Guest(_))]
// Store Record
#[test_case(s_record(create_hidden_entry(0, 100), RecordEntry::Hidden) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(
    create_hidden_entry(100, 0),
    RecordEntry::Hidden) => matches WasmErrorInner::Host(_) ; "Store Record: with hidden entry and zome out of scope")]
#[test_case(s_record(create_entry(0, 0), RecordEntry::Present(e(D::default()))) => matches WasmErrorInner::Serialize(_))]
#[test_case(s_record(create_entry(0, 100), RecordEntry::Present(e(A{}))) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(create_entry(100, 0), RecordEntry::Present(e(A{}))) => matches WasmErrorInner::Host(_))]
#[test_case(s_record(Action::Create(c(EntryType::AgentPubKey)), RecordEntry::Present(e(A{}))) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(create_entry(0, 0), RecordEntry::Present(Entry::Agent(ak(0)))) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(create_hidden_entry(0, 0), RecordEntry::Present(e(A{}))) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(create_hidden_entry(0, 100), RecordEntry::NotApplicable) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(Action::Create(c(EntryType::AgentPubKey)), RecordEntry::Hidden) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(
    Action::Create(c(EntryType::AgentPubKey)),
    RecordEntry::NotApplicable) => matches WasmErrorInner::Guest(_) ; "Store Record: Agent key with not applicable")]
#[test_case(s_record(Action::Create(c(EntryType::AgentPubKey)), RecordEntry::NotStored) => matches WasmErrorInner::Host(_))]
#[test_case(s_record(Action::Create(c(EntryType::CapClaim)), RecordEntry::Present(e(A{}))) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(Action::Create(c(EntryType::CapClaim)), RecordEntry::NotApplicable) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(Action::Create(c(EntryType::CapClaim)), RecordEntry::NotStored) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(Action::Create(c(EntryType::CapGrant)), RecordEntry::Present(e(A{}))) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(Action::Create(c(EntryType::CapGrant)), RecordEntry::NotApplicable) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(Action::Create(c(EntryType::CapGrant)), RecordEntry::NotStored) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(create_link(0, 100), RecordEntry::NotApplicable) => matches WasmErrorInner::Guest(_))]
#[test_case(s_record(create_link(100, 0), RecordEntry::NotApplicable) => matches WasmErrorInner::Host(_))]
// Store Entry
#[test_case(s_entry(c(EntryType::App(public_app_entry_def(0, 100))).into(), e(A{})) => matches WasmErrorInner::Guest(_))]
#[test_case(s_entry(c(EntryType::App(public_app_entry_def(100, 0))).into(), e(A{})) => matches WasmErrorInner::Host(_))]
#[test_case(s_entry(c(EntryType::App(public_app_entry_def(0, 0))).into(), e(D::default())) => matches WasmErrorInner::Serialize(_))]
#[test_case(s_entry(c(EntryType::App(private_app_entry_def(0, 0))).into(), e(A{})) => matches WasmErrorInner::Guest(_))]
#[test_case(s_entry(c(EntryType::CapClaim).into(), e(A{})) => matches WasmErrorInner::Guest(_))]
#[test_case(s_entry(c(EntryType::CapGrant).into(), e(A{})) => matches WasmErrorInner::Guest(_))]
// RegisterUpdate
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 0))).into(), Some(e(D::default())),
    u(EntryType::App(public_app_entry_def(0, 0))), Some(e(A{})))
    => matches WasmErrorInner::Serialize(_) ; "Register Update: original entry fails to deserialize")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 0))).into(), Some(e(A{})),
    u(EntryType::App(public_app_entry_def(0, 0))), Some(e(D::default())))
    => matches WasmErrorInner::Serialize(_) ; "Register Update: new entry fails to deserialize")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 0))).into(), None,
    u(EntryType::App(public_app_entry_def(0, 0))), Some(e(A{})))
    => matches WasmErrorInner::Guest(_) ; "Register Update: original entry is missing")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 0))).into(), Some(e(A{})),
    u(EntryType::App(public_app_entry_def(0, 0))), None)
    => matches WasmErrorInner::Guest(_) ; "Register Update: new entry is missing")]
#[test_case(r_update(
    c(EntryType::App(private_app_entry_def(0, 0))).into(), Some(e(A{})),
    u(EntryType::App(private_app_entry_def(0, 0))), None)
    => matches WasmErrorInner::Guest(_) ; "Register Update: original entry is private but also present")]
#[test_case(r_update(
    c(EntryType::App(private_app_entry_def(0, 0))).into(), None,
    u(EntryType::App(private_app_entry_def(0, 0))), Some(e(A{})))
    => matches WasmErrorInner::Guest(_) ; "Register Update: new entry is private but also present")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 100))).into(), Some(e(A{})),
    u(EntryType::App(public_app_entry_def(0, 100))), Some(e(A{})))
    => matches WasmErrorInner::Guest(_) ; "Register Update: entry type is out of range")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(100, 0))).into(), Some(e(A{})),
    u(EntryType::App(public_app_entry_def(100, 0))), Some(e(A{})))
    => matches WasmErrorInner::Host(_) ; "Register Update: zome id is out of range")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 0))).into(), Some(e(A{})),
    u(EntryType::App(private_app_entry_def(0, 0))), None)
    => matches WasmErrorInner::Guest(_) ; "Register Update: public to private type mismatch")]
#[test_case(r_update(
    c(EntryType::App(private_app_entry_def(0, 0))).into(), None,
    u(EntryType::App(public_app_entry_def(0, 0))), Some(e(A{})))
    => matches WasmErrorInner::Guest(_) ; "Register Update: private to public type mismatch")]
#[test_case(r_update(
    c(EntryType::AgentPubKey).into(), Some(e(A{})),
    u(EntryType::App(public_app_entry_def(0, 0))), Some(e(A{})))
    => matches WasmErrorInner::Guest(_) ; "Register Update: agent to app mismatch")]
#[test_case(r_update(
    c(EntryType::App(public_app_entry_def(0, 1))).into(), None,
    u(EntryType::App(public_app_entry_def(0, 0))), Some(e(A{})))
    => matches WasmErrorInner::Guest(_) ; "Register Update: entry type mismatch")]
#[test_case(r_create_link(0, 100) => matches WasmErrorInner::Guest(_) ; "Register Create Link: link type out of range")]
#[test_case(r_create_link(100, 0) => matches WasmErrorInner::Host(_) ; "Register Create Link: zome id out of range")]
#[test_case(r_delete_link(0, 100) => matches WasmErrorInner::Guest(_) ; "Register Delete Link: link type out of range")]
#[test_case(r_delete_link(100, 0) => matches WasmErrorInner::Host(_) ; "Register Delete Link: zome id out of range")]
fn op_errors(op: Op) -> WasmErrorInner {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    op.to_type::<EntryTypes, LinkTypes>().unwrap_err().error
}

// Register Agent Activity
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateEntry { action: c(EntryType::App(public_app_entry_def(0, 0))), app_entry_type: Some(UnitEntryTypes::A) }))]
// #[test_case(OpType::RegisterAgentActivity(OpActivity::CreateEntry { action: c(EntryType::App(public_app_entry_def(0, 0))), app_entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateCapClaim{ action: c(EntryType::CapClaim)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateCapGrant{ action: c(EntryType::CapGrant)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreatePrivateEntry { action: c(EntryType::App(private_app_entry_def(0, 0))), app_entry_type: Some(UnitEntryTypes::A) }))]
// #[test_case(OpType::RegisterAgentActivity(OpActivity::CreatePrivateEntry { action: c(EntryType::App(private_app_entry_def(0, 0))), app_entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateAgent { action: c(EntryType::AgentPubKey), agent: ak(0)}))]

#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateEntry { action: u(EntryType::App(public_app_entry_def(0, 0))), original_action_hash: ah(1), original_entry_hash: eh(1), app_entry_type: Some(UnitEntryTypes::A) }))]
// #[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateEntry { action: u(EntryType::App(public_app_entry_def(0, 0))), original_action_hash: ah(1), original_entry_hash: eh(1), app_entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdatePrivateEntry { action: u(EntryType::App(private_app_entry_def(0, 0))), original_action_hash: ah(1), original_entry_hash: eh(1), app_entry_type: Some(UnitEntryTypes::A)}))]
// #[test_case(OpType::RegisterAgentActivity(OpActivity::UpdatePrivateEntry { action: u(EntryType::App(private_app_entry_def(0, 0))), original_action_hash: ah(1), original_entry_hash: eh(1), app_entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateAgent { action: u(EntryType::AgentPubKey), new_key: ak(0), original_action_hash: ah(1), original_key: ak(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateCapClaim { action: u(EntryType::CapClaim), original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateCapGrant { action: u(EntryType::CapGrant), original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::DeleteEntry { action: d(ah(0)), original_action_hash: ah(0), original_entry_hash: eh(0) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateLink { action: cl(0, 0), base_address: lh(0), target_address: lh(1), tag: ().into(), link_type: Some(LinkTypes::A)}))]
// #[test_case(OpType::RegisterAgentActivity(OpActivity::CreateLink { action: cl(0, 0), base_address: lh(0), target_address: lh(1), tag: ().into(), link_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::DeleteLink{ action: dl(ah(0)), original_action_hash: ah(0), base_address: eh(0).into()}))]
// Action's without entries
#[test_case(OpType::RegisterAgentActivity(OpActivity::Dna { action: dna(dh(0)), dna_hash: dh(0)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::OpenChain { previous_dna_hash: dh(0), action: oc(dh(0))}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CloseChain { new_dna_hash: dh(0), action: cc(dh(0))}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::InitZomesComplete { action: izc()}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::AgentValidationPkg{ membrane_proof: None, action: avp(None) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::AgentValidationPkg{ membrane_proof: Some(mp()), action: avp(Some(mp())) }))]
// Store Record
// Entries
// App Entries
#[test_case(OpType::StoreRecord(OpRecord::CreateEntry { action: c(EntryType::App(public_app_entry_def(0, 0))), app_entry: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::CreateEntry { action: c(EntryType::App(public_app_entry_def(0, 2))), app_entry: EntryTypes::C(C{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), app_entry: EntryTypes::A(A{}), action: u(EntryType::App(public_app_entry_def(0, 0))) }))]
#[test_case(OpType::StoreRecord(OpRecord::DeleteEntry { original_action_hash: ah(1), original_entry_hash: eh(0), action: d(ah(1)) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), app_entry: EntryTypes::C(C{}), action: u(EntryType::App(public_app_entry_def(0, 2))) }))]
// Agent Keys
#[test_case(OpType::StoreRecord(OpRecord::CreateAgent{ action: c(EntryType::AgentPubKey), agent: ak(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateAgent { action: u(EntryType::AgentPubKey), original_key: ak(1), new_key: ak(0), original_action_hash: ah(1) }))]
// Private Entries
#[test_case(OpType::StoreRecord(OpRecord::CreatePrivateEntry { action: c(EntryType::App(private_app_entry_def(0, 0))), app_entry_type: UnitEntryTypes::A }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdatePrivateEntry { action: u(EntryType::App(private_app_entry_def(0, 0))), original_action_hash: ah(1), original_entry_hash: eh(1), app_entry_type: UnitEntryTypes::A }))]
// Caps
#[test_case(OpType::StoreRecord(OpRecord::CreateCapClaim{ action: c(EntryType::CapClaim)}))]
#[test_case(OpType::StoreRecord(OpRecord::CreateCapGrant{ action: c(EntryType::CapGrant)}))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateCapClaim{ action: u(EntryType::CapClaim), original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateCapGrant{ action: u(EntryType::CapGrant), original_action_hash: ah(1), original_entry_hash: eh(1) }))]
// Links
#[test_case(OpType::StoreRecord(OpRecord::CreateLink { action: cl(0, 0), base_address: lh(0), target_address: lh(1), tag: ().into(), link_type: LinkTypes::A }))]
#[test_case(OpType::StoreRecord(OpRecord::DeleteLink { action: dl(ah(0)), original_action_hash: ah(0), base_address: eh(0).into() }))]
// Action's without entries
#[test_case(OpType::StoreRecord(OpRecord::Dna{ action: dna(dh(0)), dna_hash: dh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::OpenChain{ action: oc(dh(0)), previous_dna_hash: dh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::CloseChain{ action: cc(dh(1)), new_dna_hash: dh(1)}))]
#[test_case(OpType::StoreRecord(OpRecord::InitZomesComplete { action: izc() }))]
#[test_case(OpType::StoreRecord(OpRecord::AgentValidationPkg { action: avp(None), membrane_proof: None}))]
#[test_case(OpType::StoreRecord(OpRecord::AgentValidationPkg { action: avp(Some(mp())), membrane_proof: Some(mp())}))]
// Store Entry
#[test_case(OpType::StoreEntry(OpEntry::CreateEntry { action: EntryCreationAction::Create(c(EntryType::App(public_app_entry_def(0, 0)))), app_entry: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreEntry(OpEntry::UpdateEntry { action: u(EntryType::App(public_app_entry_def(0, 0))), original_action_hash: ah(1), original_entry_hash: eh(1), app_entry: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreEntry(OpEntry::CreateAgent { action: EntryCreationAction::Create(c(EntryType::AgentPubKey)), agent: ak(0)}))]
#[test_case(OpType::StoreEntry(OpEntry::UpdateAgent { action: u(EntryType::AgentPubKey), original_key: ak(1), new_key: ak(0), original_action_hash: ah(1) }))]
// // Error Cases
// // #[test_case(OpType::StoreEntry(OpEntry::CreateEntry {entry_hash: eh(0), entry_type: EntryTypes::B(B{}) }))]
// Register Update
#[test_case(OpType::RegisterUpdate(OpUpdate::Entry { action: u(EntryType::App(public_app_entry_def(0, 0))), original_action_hash: ah(1), app_entry: EntryTypes::A(A{}), original_app_entry: EntryTypes::A(A{}) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::PrivateEntry { action: u(EntryType::App(private_app_entry_def(0, 0))),  original_action_hash: ah(1), app_entry_type: UnitEntryTypes::A, original_app_entry_type: UnitEntryTypes::A }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::Agent { action: u(EntryType::AgentPubKey), original_key: ak(1), new_key: ak(0), original_action_hash: ah(1) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::CapClaim { action: u(EntryType::CapClaim), original_action_hash: ah(1) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::CapGrant { action: u(EntryType::CapGrant), original_action_hash: ah(1) }))]
// Register Delete
#[test_case(OpType::RegisterDelete(OpDelete::Entry { action: d(ah(0)), original_action: EntryCreationAction::Create(c(EntryType::App(public_app_entry_def(0, 0)))), original_app_entry: EntryTypes::A(A{}) }))]
#[test_case(OpType::RegisterDelete(OpDelete::PrivateEntry { action: d(ah(0)), original_action: EntryCreationAction::Create(c(EntryType::App(private_app_entry_def(0, 0)))), original_app_entry_type: UnitEntryTypes::A }))]
#[test_case(OpType::RegisterDelete(OpDelete::Agent { action: d(ah(1)), original_key: ak(0), original_action: EntryCreationAction::Create(c(EntryType::AgentPubKey)) }))]
#[test_case(OpType::RegisterDelete(OpDelete::CapClaim { action: d(ah(1)), original_action: EntryCreationAction::Create(c(EntryType::CapClaim)) }))]
#[test_case(OpType::RegisterDelete(OpDelete::CapGrant { action: d(ah(1)), original_action: EntryCreationAction::Create(c(EntryType::CapGrant))  }))]
// Register Create Link
#[test_case(OpType::RegisterCreateLink { action: cl(0, 0), base_address: lh(0), target_address: lh(1), tag: ().into(), link_type: LinkTypes::A })]
#[test_case(OpType::RegisterCreateLink { action: cl(0, 1), base_address: lh(0), target_address: lh(1), tag: ().into(), link_type: LinkTypes::B })]
// Register Delete Link
#[test_case(OpType::RegisterDeleteLink { action: dl(ah(0)), original_action_hash: ah(0), base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::A })]
#[test_case(OpType::RegisterDeleteLink { action: dl(ah(0)), original_action_hash: ah(0), base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::C })]
fn op_to_type(op: OpType<EntryTypes, LinkTypes>) {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    let data = vec![0u8; 2000];
    let mut ud = Unstructured::new(&data);
    let o = match op.clone() {
        OpType::StoreRecord(OpRecord::Dna { action, .. }) => {
            let d = Action::Dna(action);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::AgentValidationPkg { action, .. }) => {
            let d = Action::AgentValidationPkg(action);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::InitZomesComplete { action }) => {
            let d = Action::InitZomesComplete(action);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::OpenChain {
            action, ..
        }) => {
            let d = Action::OpenChain(action);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::CloseChain { action, .. }) => {
            let d = Action::CloseChain(action);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::CreateCapClaim { action }) => {
            let d = Action::Create(action);
            store_record_entry(d, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateCapGrant { action }) => {
            let d = Action::Create(action);
            store_record_entry(d, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateCapClaim {
            action,
            ..
        }) => {
            let u = Action::Update(action);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateCapGrant {
            action,
            ..
        }) => {
            let u = Action::Update(action);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateEntry {
            app_entry: et,
            action,
        }) => {
            let entry = RecordEntry::Present(Entry::try_from(&et).unwrap());
            let c = Action::Create(action);
            store_record_entry(c, entry)
        }
        OpType::StoreRecord(OpRecord::CreatePrivateEntry {
            action,
            ..
        }) => {
            let c = Action::Create(action);
            store_record_entry(c, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateAgent { action, agent }) => {
            let entry = RecordEntry::Present(Entry::Agent(agent.clone()));
            let c = Action::Create(action);
            store_record_entry(c, entry)
        }
        OpType::StoreRecord(OpRecord::CreateLink {
            action,
            ..
        }) => {
            let c = Action::CreateLink(action);
            Op::StoreRecord(StoreRecord {
                record: Record {
                    signed_action: SignedHashed {
                        hashed: ActionHashed::from_content_sync(c),
                        signature: Signature::arbitrary(&mut ud).unwrap(),
                    },
                    entry: RecordEntry::NotApplicable,
                },
            })
        }
        OpType::StoreRecord(OpRecord::DeleteLink {
            action,
            ..
        }) => {
            let c = Action::DeleteLink(action);
            Op::StoreRecord(StoreRecord {
                record: Record {
                    signed_action: SignedHashed {
                        hashed: ActionHashed::from_content_sync(c),
                        signature: Signature::arbitrary(&mut ud).unwrap(),
                    },
                    entry: RecordEntry::NotApplicable,
                },
            })
        }
        OpType::StoreRecord(OpRecord::UpdateEntry {
            app_entry: et,
            action,
            ..
        }) => {
            let entry = RecordEntry::Present(Entry::try_from(&et).unwrap());
            let u = Action::Update(action);
            store_record_entry(u, entry)
        }
        OpType::StoreRecord(OpRecord::UpdatePrivateEntry {
            action,
            ..
        }) => {
            let u = Action::Update(action);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateAgent {
            new_key,
            action,
            ..
        }) => {
            let entry = RecordEntry::Present(Entry::Agent(new_key.clone()));
            let u = Action::Update(action);
            store_record_entry(u, entry)
        }
        OpType::StoreRecord(OpRecord::DeleteEntry {
            action,
            ..
        }) => {
            let d = Action::Delete(action);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreEntry(OpEntry::CreateEntry {
            app_entry: et,
            action,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            store_entry_entry(action, entry)
        }
        OpType::StoreEntry(OpEntry::UpdateEntry {
            app_entry: et,
            action,
            ..
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let u = EntryCreationAction::Update(action);
            store_entry_entry(u, entry)
        }
        OpType::StoreEntry(OpEntry::CreateAgent { action, agent }) => {
            let entry = Entry::Agent(agent.clone());
            store_entry_entry(action, entry)
        }
        OpType::StoreEntry(OpEntry::UpdateAgent {
            new_key,
            action,
            ..
        }) => {
            let entry = Entry::Agent(new_key.clone());
            let u = EntryCreationAction::Update(action);
            store_entry_entry(u, entry)
        }
        OpType::RegisterCreateLink {
            action,
            ..
        } => {
            Op::RegisterCreateLink(RegisterCreateLink {
                create_link: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
            })
        }
        OpType::RegisterDeleteLink {
            original_action_hash: _,
            link_type: lt,
            base_address,
            target_address,
            tag,
            action,
        } => {
            let t = ScopedLinkType::try_from(&lt).unwrap();
            let mut c = CreateLink::arbitrary(&mut ud).unwrap();
            c.zome_index = t.zome_index;
            c.link_type = t.zome_type;
            c.base_address = base_address;
            c.target_address = target_address;
            c.tag = tag;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                create_link: c,
            })
        }
        OpType::RegisterUpdate(OpUpdate::Entry {
            original_action_hash,
            original_app_entry: oet,
            app_entry: et,
            action,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let original_entry = Entry::try_from(&oet).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let original_action = update(
                (&oet).into(),
                &mut ud,
                t,
                action.entry_hash.clone(),
                original_action_hash.clone(),
                action.original_entry_address.clone(),
            );
            let original_action = EntryCreationAction::Update(original_action);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: Some(entry),
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterUpdate(OpUpdate::Agent {
            original_key,
            new_key,
            action,
            ..
        }) => {
            let entry = Entry::Agent(new_key.clone());
            let original_entry = Entry::Agent(original_key.clone());
            let c = Create::arbitrary(&mut ud).unwrap();
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: Some(entry),
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterUpdate(OpUpdate::PrivateEntry {
            original_action_hash: _,
            original_app_entry_type: _,
            app_entry_type: et,
            action,
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let original_action = create(
                EntryVisibility::Private,
                &mut ud,
                t,
                action.entry_hash.clone(),
            );
            let original_action = EntryCreationAction::Create(original_action);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: None,
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterUpdate(OpUpdate::CapClaim {
            original_action_hash: _,
            action,
        }) => {
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::CapClaim;
            c.entry_hash = action.original_entry_address.clone();
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: None,
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterUpdate(OpUpdate::CapGrant {
            action,
            ..
        }) => {
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::CapGrant;
            c.entry_hash = action.original_entry_address.clone();
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: None,
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterDelete(OpDelete::Entry {
            original_action,
            original_app_entry: original_et,
            action,
        }) => {
            let original_entry = Entry::try_from(&original_et).unwrap();
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterDelete(OpDelete::Agent {
            original_action,
            original_key,
            action,
        }) => {
            let original_entry = Entry::Agent(original_key.clone());
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterDelete(OpDelete::PrivateEntry {
            original_action,
            original_app_entry_type: _,
            action,
        }) => {
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterDelete(OpDelete::CapClaim {
            original_action,
            action,
        }) => {
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterDelete(OpDelete::CapGrant {
            original_action,
            action,
        }) => {
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(action),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterAgentActivity(activity) => {
            let r = match activity {
                OpActivity::CreateEntry {
                    action,
                    app_entry_type: _,
                } => Action::Create(action),
                OpActivity::CreatePrivateEntry {
                    action,
                    app_entry_type: _,
                } => Action::Create(action),
                OpActivity::CreateAgent { action, .. } => {
                    Action::Create(action)
                }
                OpActivity::UpdateEntry {
                    action,
                    ..
                } => {
                    Action::Update(action)
                }
                OpActivity::UpdatePrivateEntry {
                    action,
                    ..
                } => {
                    Action::Update(action)
                }
                OpActivity::UpdateAgent {
                    action,
                    ..
                } => {
                    Action::Update(action)
                }
                OpActivity::DeleteEntry {
                    action,
                    ..
                } => {
                    Action::Delete(action)
                }
                OpActivity::CreateLink {
                    action,
                    ..
                } => {
                    Action::CreateLink(action)
                }
                OpActivity::DeleteLink {
                    action,
                    ..
                } => {
                    Action::DeleteLink(action)
                }
                OpActivity::CreateCapClaim { action } => {
                    Action::Create(action)
                }
                OpActivity::CreateCapGrant { action } => {
                    Action::Create(action)
                }
                OpActivity::UpdateCapClaim {
                    action,
                    ..
                } => {
                    Action::Update(action)
                }
                OpActivity::UpdateCapGrant {
                    action,
                    ..
                } => {
                    Action::Update(action)
                }
                OpActivity::Dna { action, .. } => {
                    Action::Dna(action)
                }
                OpActivity::OpenChain {
                    action,
                    ..
                } => {
                    Action::OpenChain(action)
                }
                OpActivity::CloseChain { action, .. } => {
                    Action::CloseChain(action)
                }
                OpActivity::AgentValidationPkg { action, .. } => {
                    Action::AgentValidationPkg(action)
                }
                OpActivity::InitZomesComplete { action } => {
                    Action::InitZomesComplete(action)
                }
            };
            let r = RegisterAgentActivity {
                cached_entry: None,
                action: SignedHashed {
                    hashed: HoloHashed::from_content_sync(r),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
            };
            Op::RegisterAgentActivity(r)
        }
    };
    assert_eq!(o.to_type().unwrap(), op);
}

fn store_record_entry(action: Action, entry: RecordEntry) -> Op {
    Op::StoreRecord(StoreRecord {
        record: Record {
            signed_action: SignedHashed {
                hashed: ActionHashed::from_content_sync(action),
                signature: Signature([0u8; 64]),
            },
            entry,
        },
    })
}
fn store_entry_entry(action: EntryCreationAction, entry: Entry) -> Op {
    Op::StoreEntry(StoreEntry {
        action: SignedHashed {
            hashed: HoloHashed::from_content_sync(action),
            signature: Signature([0u8; 64]),
        },
        entry,
    })
}

fn create(
    visibility: EntryVisibility,
    ud: &mut Unstructured,
    t: ScopedEntryDefIndex,
    entry_hash: EntryHash,
) -> Create {
    let mut c = Create::arbitrary(ud).unwrap();
    c.entry_type = EntryType::App(AppEntryDef {
        entry_index: t.zome_type,
        zome_index: t.zome_index,
        visibility,
    });
    c.entry_hash = entry_hash;
    c
}
fn update(
    visibility: EntryVisibility,
    ud: &mut Unstructured,
    t: ScopedEntryDefIndex,
    entry_hash: EntryHash,
    original_action_hash: ActionHash,
    original_entry_hash: EntryHash,
) -> Update {
    let mut u = Update::arbitrary(ud).unwrap();
    u.entry_type = EntryType::App(AppEntryDef {
        entry_index: t.zome_type,
        zome_index: t.zome_index,
        visibility,
    });
    u.entry_hash = entry_hash;
    u.original_action_address = original_action_hash;
    u.original_entry_address = original_entry_hash;
    u
}

#[test]
fn op_match_sanity() {
    fn empty_create() -> Create {
        Create {
            author: AgentPubKey::from_raw_36(vec![0u8; 36]),
            timestamp: Timestamp(0),
            action_seq: 1,
            prev_action: ActionHash::from_raw_36(vec![0u8; 36]),
            entry_type: EntryType::App(AppEntryDef {
                entry_index: 0.into(),
                zome_index: 0.into(),
                visibility: EntryVisibility::Public,
            }),
            entry_hash: EntryHash::from_raw_36(vec![0u8; 36]),
            weight: Default::default(),
        }
    }
    let op = Op::StoreRecord(StoreRecord {
        record: Record {
            signed_action: SignedHashed {
                hashed: ActionHashed {
                    content: Action::Create(Create {
                        entry_type: EntryType::App(AppEntryDef {
                            entry_index: 0.into(),
                            zome_index: 0.into(),
                            visibility: EntryVisibility::Public,
                        }),
                        ..empty_create()
                    }),
                    hash: ActionHash::from_raw_36(vec![1u8; 36]),
                },
                signature: Signature([0u8; 64]),
            },
            entry: RecordEntry::Present(EntryTypes::A(A {}).try_into().unwrap()),
        },
    });
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    match op.to_type().unwrap() {
        OpType::StoreRecord(r) => match r {
            OpRecord::CreateEntry {
                app_entry: EntryTypes::A(_),
                action: _,
            } => (),
            OpRecord::CreateEntry {
                app_entry: EntryTypes::B(_),
                action: _,
            } => unreachable!(),
            OpRecord::CreateEntry {
                app_entry: EntryTypes::C(_),
                action: _,
            } => (),
            OpRecord::CreatePrivateEntry {
                app_entry_type: UnitEntryTypes::B,
                action: _,
            } => (),
            OpRecord::CreatePrivateEntry {
                app_entry_type: _, ..
            } => unreachable!(),
            OpRecord::CreateAgent { .. } => (),
            OpRecord::CreateCapClaim { .. } => (),
            OpRecord::CreateCapGrant { .. } => (),
            OpRecord::UpdateEntry {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
                app_entry: EntryTypes::A(_),
            } => (),
            OpRecord::UpdateEntry {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
                app_entry: EntryTypes::B(_),
            } => unreachable!(),
            OpRecord::UpdateEntry {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
                app_entry: EntryTypes::C(_),
            } => (),
            OpRecord::UpdatePrivateEntry {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
                app_entry_type: UnitEntryTypes::B,
            } => (),
            OpRecord::UpdatePrivateEntry { .. } => unreachable!(),
            OpRecord::UpdateAgent {
                action: _,
                original_action_hash: _,
                original_key: _,
                new_key: _,
            } => (),
            OpRecord::UpdateCapClaim {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
            } => (),
            OpRecord::UpdateCapGrant {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
            } => (),
            OpRecord::DeleteEntry {
                action: _,
                original_action_hash: _,
                original_entry_hash: _,
            } => (),
            OpRecord::CreateLink {
                base_address: _,
                target_address: _,
                tag: _,
                link_type: LinkTypes::A,
                action: _,
            } => (),
            OpRecord::CreateLink {
                base_address: _,
                target_address: _,
                tag: _,
                link_type: LinkTypes::B,
                action: _,
            } => (),
            OpRecord::CreateLink {
                base_address: _,
                target_address: _,
                tag: _,
                link_type: LinkTypes::C,
                action: _,
            } => (),
            OpRecord::DeleteLink { .. } => (),
            OpRecord::Dna { .. } => (),
            OpRecord::OpenChain { .. } => (),
            OpRecord::CloseChain { .. } => (),
            OpRecord::AgentValidationPkg { .. } => (),
            OpRecord::InitZomesComplete { .. } => (),
        },
        OpType::StoreEntry(_) => (),
        OpType::RegisterAgentActivity(_) => (),
        OpType::RegisterCreateLink {
            action: _,
            base_address: _,
            target_address: _,
            tag: _,
            link_type,
        } => match link_type {
            LinkTypes::A => (),
            LinkTypes::B => (),
            LinkTypes::C => (),
        },
        OpType::RegisterDeleteLink {
            original_action_hash: _,
            base_address: _,
            target_address: _,
            tag: _,
            link_type,
            action: _,
        } => match link_type {
            LinkTypes::A => (),
            LinkTypes::B => (),
            LinkTypes::C => (),
        },
        OpType::RegisterUpdate(_) => (),
        OpType::RegisterDelete(_) => (),
    }
    match op.to_type::<_, ()>().unwrap() {
        OpType::StoreRecord(OpRecord::CreateEntry {
            action: _,
            app_entry: EntryTypes::A(_),
        }) => (),
        _ => (),
    }
    match op.to_type::<(), _>().unwrap() {
        OpType::StoreRecord(OpRecord::CreateLink {
            link_type: LinkTypes::A,
            ..
        }) => (),
        _ => (),
    }
    match op.to_type::<(), ()>().unwrap() {
        _ => (),
    }
}
