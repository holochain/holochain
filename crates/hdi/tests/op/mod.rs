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
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateEntry { action: c(A), entry_type: Some(UnitEntryTypes::A) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateEntry { action: c(A), entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateCapClaim{ entry_hash: eh(0)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateCapGrant{ entry_hash: eh(0)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreatePrivateEntry {entry_type: Some(UnitEntryTypes::A) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreatePrivateEntry {entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateAgent { agent: ak(4)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: Some(UnitEntryTypes::A) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdatePrivateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: Some(UnitEntryTypes::A) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdatePrivateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateAgent { new_key: ak(2), original_action_hash: ah(1), original_key: ak(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateCapClaim { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::UpdateCapGrant { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::DeleteEntry { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: Some(LinkTypes::A) }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: None }))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::DeleteLink{ original_action_hash: ah(4)}))]
// Action's without entries
#[test_case(OpType::RegisterAgentActivity(OpActivity::Dna { dna_hash: dh(0)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::OpenChain { previous_dna_hash: dh(0)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::CloseChain { new_dna_hash: dh(0)}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::InitZomesComplete {}))]
#[test_case(OpType::RegisterAgentActivity(OpActivity::AgentValidationPkg{None}))]
// Store Record
// Entries
// App Entries
#[test_case(OpType::StoreRecord(OpRecord::CreateEntry { entry_type: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::DeleteEntry { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::StoreRecord(OpRecord::CreateEntry { entry_type: EntryTypes::C(C{}) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: EntryTypes::C(C{}) }))]
// Agent Keys
#[test_case(OpType::StoreRecord(OpRecord::CreateAgent{ agent: ak(4)}))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateAgent { original_key: ak(4), new_key: ak(8), original_action_hash: ah(2) }))]
// Private Entries
#[test_case(OpType::StoreRecord(OpRecord::CreatePrivateEntry { entry_type: UnitEntryTypes::A }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdatePrivateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: UnitEntryTypes::A }))]
// Caps
#[test_case(OpType::StoreRecord(OpRecord::CreateCapClaim{ entry_hash: eh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::CreateCapGrant{ entry_hash: eh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateCapClaim{ original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::StoreRecord(OpRecord::UpdateCapGrant{ original_action_hash: ah(1), original_entry_hash: eh(1) }))]
// Links
#[test_case(OpType::StoreRecord(OpRecord::CreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::A }))]
#[test_case(OpType::StoreRecord(OpRecord::DeleteLink{ original_action_hash: ah(4)}))]
// Action's without entries
#[test_case(OpType::StoreRecord(OpRecord::Dna{ dna_hash: dh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::OpenChain{ previous_dna_hash: dh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::CloseChain{new_dna_hash: dh(0)}))]
#[test_case(OpType::StoreRecord(OpRecord::InitZomesComplete {}))]
#[test_case(OpType::StoreRecord(OpRecord::AgentValidationPkg {None}))]
// Store Entry
#[test_case(OpType::StoreEntry(OpEntry::CreateEntry { entry_type: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreEntry(OpEntry::UpdateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), entry_type: EntryTypes::A(A{}) }))]
#[test_case(OpType::StoreEntry(OpEntry::CreateAgent { agent: ak(4)}))]
#[test_case(OpType::StoreEntry(OpEntry::UpdateAgent { original_key: ak(4), new_key: ak(8), original_action_hash: ah(2) }))]
// Error Cases
// #[test_case(OpType::StoreEntry(OpEntry::CreateEntry {entry_hash: eh(0), entry_type: EntryTypes::B(B{}) }))]
// Register Update
#[test_case(OpType::RegisterUpdate(OpUpdate::Entry { original_action_hash: ah(1), original_entry_hash: eh(1), new_entry_type: EntryTypes::A(A{}), original_entry_type: EntryTypes::A(A{}) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::PrivateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), new_entry_type: UnitEntryTypes::A, original_entry_type: UnitEntryTypes::A }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::Agent { original_key: ak(4), new_key: ak(8), original_action_hash: ah(2) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::CapClaim { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterUpdate(OpUpdate::CapGrant { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
// Register Delete
#[test_case(OpType::RegisterDelete(OpDelete::Entry { original_action_hash: ah(1), original_entry_hash: eh(1), original_entry_type: EntryTypes::A(A{}) }))]
#[test_case(OpType::RegisterDelete(OpDelete::PrivateEntry { original_action_hash: ah(1), original_entry_hash: eh(1), original_entry_type: UnitEntryTypes::A }))]
#[test_case(OpType::RegisterDelete(OpDelete::Agent { original_key: ak(4), original_action_hash: ah(2) }))]
#[test_case(OpType::RegisterDelete(OpDelete::CapClaim { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
#[test_case(OpType::RegisterDelete(OpDelete::CapGrant { original_action_hash: ah(1), original_entry_hash: eh(1) }))]
// Register Create Link
#[test_case(OpType::RegisterCreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::A })]
#[test_case(OpType::RegisterCreateLink {base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::B })]
// Register Delete Link
#[test_case(OpType::RegisterDeleteLink { original_action_hash: ah(2), base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::A })]
#[test_case(OpType::RegisterDeleteLink { original_action_hash: ah(2), base_address: lh(0), target_address: lh(2), tag: ().into(), link_type: LinkTypes::C })]
fn op_to_type(op: OpType<EntryTypes, LinkTypes>) {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    let data = vec![0u8; 2000];
    let mut ud = Unstructured::new(&data);
    let o = match op.clone() {
        OpType::StoreRecord(OpRecord::Dna{dna_hash, ..}) => {
            let mut d = Dna::arbitrary(&mut ud).unwrap();
            d.hash = dna_hash;
            let d = Action::Dna(d);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::AgentValidationPkg{membrane_proof, ..}) => {
            let mut d = AgentValidationPkg::arbitrary(&mut ud).unwrap();
            d.membrane_proof = membrane_proof;
            let d = Action::AgentValidationPkg(d);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::InitZomesComplete{..}) => {
            let d = InitZomesComplete::arbitrary(&mut ud).unwrap();
            let d = Action::InitZomesComplete(d);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::OpenChain{previous_dna_hash, ..}) => {
            let mut d = OpenChain::arbitrary(&mut ud).unwrap();
            d.prev_dna_hash = previous_dna_hash;
            let d = Action::OpenChain(d);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::CloseChain {new_dna_hash, ..}) => {
            let mut d = CloseChain::arbitrary(&mut ud).unwrap();
            d.new_dna_hash = new_dna_hash;
            let d = Action::CloseChain(d);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreRecord(OpRecord::CreateCapClaim {action}) => {
            let mut d = Create::arbitrary(&mut ud).unwrap();
            d.entry_hash = action.entry_hash;
            d.entry_type = EntryType::CapClaim;
            let d = Action::Create(d);
            store_record_entry(d, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateCapGrant {action}) => {
            let mut d = Create::arbitrary(&mut ud).unwrap();
            d.entry_hash = action.entry_hash;
            d.entry_type = EntryType::CapGrant;
            let d = Action::Create(d);
            store_record_entry(d, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateCapClaim {
            original_action_hash,
            original_entry_hash,
            action,
        }) => {
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_hash = action.entry_hash;
            u.entry_type = EntryType::CapClaim;
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_entry_hash;
            let u = Action::Update(u);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateCapGrant {
            original_action_hash,
            original_entry_hash,
            action,
        }) => {
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_hash = action.entry_hash;
            u.entry_type = EntryType::CapGrant;
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_entry_hash;
            let u = Action::Update(u);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateEntry {
            app_entry: et,
            action,
        }) => {
            let entry = RecordEntry::Present(Entry::try_from(&et).unwrap());
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let c = create((&et).into(), &mut ud, t, action.entry_hash);
            let c = Action::Create(c);
            store_record_entry(c, entry)
        }
        OpType::StoreRecord(OpRecord::CreatePrivateEntry {
            app_entry_type: et,
            action,
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let c = create(EntryVisibility::Private, &mut ud, t, action.entry_hash);
            let c = Action::Create(c);
            store_record_entry(c, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::CreateAgent{agent, ..}) => {
            let entry = RecordEntry::Present(Entry::Agent(agent.clone()));
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::AgentPubKey;
            c.entry_hash = agent.into();
            let c = Action::Create(c);
            store_record_entry(c, entry)
        }
        OpType::StoreRecord(OpRecord::CreateLink {
            link_type: lt,
            base_address,
            target_address,
            tag,
            ..
        }) => {
            let t = ScopedLinkType::try_from(&lt).unwrap();
            let mut c = CreateLink::arbitrary(&mut ud).unwrap();
            c.zome_index = t.zome_index;
            c.link_type = t.zome_type;
            c.base_address = base_address;
            c.target_address = target_address;
            c.tag = tag;
            let c = Action::CreateLink(c);
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
        OpType::StoreRecord(OpRecord::DeleteLink{original_action_hash, ..}) => {
            let mut c = DeleteLink::arbitrary(&mut ud).unwrap();
            c.link_add_address = original_action_hash;
            let c = Action::DeleteLink(c);
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
            original_action_hash,
            original_entry_hash,
            app_entry: et,
            action,
        }) => {
            let entry = RecordEntry::Present(Entry::try_from(&et).unwrap());
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let u = update(
                (&et).into(),
                &mut ud,
                t,
                action.entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            let u = Action::Update(u);
            store_record_entry(u, entry)
        }
        OpType::StoreRecord(OpRecord::UpdatePrivateEntry {
            original_action_hash,
            original_entry_hash,
            app_entry_type: et,
            action
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let u = update(
                EntryVisibility::Private,
                &mut ud,
                t,
                action.entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            let u = Action::Update(u);
            store_record_entry(u, RecordEntry::Hidden)
        }
        OpType::StoreRecord(OpRecord::UpdateAgent {
            original_action_hash,
            original_key,
            new_key,
            action,
        }) => {
            let entry = RecordEntry::Present(Entry::Agent(new_key.clone()));
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::AgentPubKey;
            u.entry_hash = new_key.into();
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_key.into();
            let u = Action::Update(u);
            store_record_entry(u, entry)
        }
        OpType::StoreRecord(OpRecord::DeleteEntry {
            original_action_hash,
            original_entry_hash,
            action,
        }) => {
            let mut d = Delete::arbitrary(&mut ud).unwrap();
            d.deletes_address = original_action_hash;
            d.deletes_entry_address = original_entry_hash;
            let d = Action::Delete(d);
            store_record_entry(d, RecordEntry::NotApplicable)
        }
        OpType::StoreEntry(OpEntry::CreateEntry {
            app_entry: et,
            action,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let c = create(EntryVisibility::Public, &mut ud, t, action.entry_hash);
            let c = EntryCreationAction::Create(c);
            store_entry_entry(c, entry)
        }
        OpType::StoreEntry(OpEntry::UpdateEntry {
            original_action_hash,
            original_entry_hash,
            app_entry: et,
            action,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let u = update(
                (&et).into(),
                &mut ud,
                t,
                action.entry_hash,
                original_action_hash,
                original_entry_hash,
            );
            let u = EntryCreationAction::Update(u);
            store_entry_entry(u, entry)
        }
        OpType::StoreEntry(OpEntry::CreateAgent{agent, ..}) => {
            let entry = Entry::Agent(agent.clone());
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::AgentPubKey;
            c.entry_hash = agent.into();
            let c = EntryCreationAction::Create(c);
            store_entry_entry(c, entry)
        }
        OpType::StoreEntry(OpEntry::UpdateAgent {
            original_action_hash,
            original_key,
            new_key,
            action,
        }) => {
            let entry = Entry::Agent(new_key.clone());
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::AgentPubKey;
            u.entry_hash = new_key.into();
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_key.into();
            let u = EntryCreationAction::Update(u);
            store_entry_entry(u, entry)
        }
        OpType::RegisterCreateLink {
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
            Op::RegisterCreateLink(RegisterCreateLink {
                create_link: SignedHashed {
                    hashed: HoloHashed::from_content_sync(c),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
            })
        }
        OpType::RegisterDeleteLink {
            original_action_hash,
            link_type: lt,
            base_address,
            target_address,
            tag,
            action,
        } => {
            let t = ScopedLinkType::try_from(&lt).unwrap();
            let mut c = CreateLink::arbitrary(&mut ud).unwrap();
            let mut d = DeleteLink::arbitrary(&mut ud).unwrap();
            d.link_add_address = original_action_hash;
            c.zome_index = t.zome_index;
            c.link_type = t.zome_type;
            c.base_address = base_address;
            c.target_address = target_address;
            c.tag = tag;
            Op::RegisterDeleteLink(RegisterDeleteLink {
                delete_link: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                create_link: c,
            })
        }
        OpType::RegisterUpdate(OpUpdate::Entry {
            original_action_hash,
            original_app_entry,
            app_entry: et,
            action,
        }) => {
            let entry = Entry::try_from(&et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let original_action = update(
                original_app_entry.into(),
                &mut ud,
                t,
                action.entry_hash.clone(),
                original_action_hash.clone(),
                original_app_entry.entry_hash.clone(),
            );
            let original_action = EntryCreationAction::Update(original_action);
            let u = update(
                (&et).into(),
                &mut ud,
                t,
                action.entry_hash,
                original_action_hash,
                original_app_entry.entry_hash,
            );
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: Some(entry),
                original_action,
                original_entry: Some(original_app_entry),
            })
        }
        OpType::RegisterUpdate(OpUpdate::Agent {
            original_action_hash,
            original_key,
            new_key,
            action,
        }) => {
            let entry = Entry::Agent(new_key.clone());
            let original_entry = Entry::Agent(original_key.clone());
            let mut u = Update::arbitrary(&mut ud).unwrap();
            let c = Create::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::AgentPubKey;
            u.entry_hash = new_key.into();
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_key.into();
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: Some(entry),
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterUpdate(OpUpdate::PrivateEntry {
            original_action_hash,
            original_app_entry_type,
            app_entry_type: et,
            action,
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let original_action = create(EntryVisibility::Private, &mut ud, t, action.entry_hash.clone());
            let original_action = EntryCreationAction::Create(original_action);
            let u = update(
                EntryVisibility::Private,
                &mut ud,
                t,
                action.entry_hash,
                original_action_hash,
                original_action.entry_hash,
            );
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: None,
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterUpdate(OpUpdate::CapClaim {
            original_action_hash,
            original_entry_hash,
            action,
        }) => {
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::CapClaim;
            u.entry_hash = action.entry_hash;
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_entry_hash.clone();
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::CapClaim;
            c.entry_hash = original_entry_hash;
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: None,
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterUpdate(OpUpdate::CapGrant {
            entry_hash,
            original_action_hash,
            original_entry_hash,
        }) => {
            let mut u = Update::arbitrary(&mut ud).unwrap();
            u.entry_type = EntryType::CapGrant;
            u.entry_hash = entry_hash;
            u.original_action_address = original_action_hash;
            u.original_entry_address = original_entry_hash.clone();
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::CapGrant;
            c.entry_hash = original_entry_hash;
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterUpdate(RegisterUpdate {
                update: SignedHashed {
                    hashed: HoloHashed::from_content_sync(u),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                new_entry: None,
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterDelete(OpDelete::Entry {
            original_action_hash,
            original_entry_hash,
            original_entry_type: original_et,
        }) => {
            let original_entry = Entry::try_from(&original_et).unwrap();
            let t = ScopedEntryDefIndex::try_from(&original_et).unwrap();
            let mut d = Delete::arbitrary(&mut ud).unwrap();
            d.deletes_address = original_action_hash;
            d.deletes_entry_address = original_entry_hash.clone();
            let original_action = create(
                (&original_et).into(),
                &mut ud,
                t,
                original_entry_hash.clone(),
            );
            let original_action = EntryCreationAction::Create(original_action);
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterDelete(OpDelete::Agent {
            original_action_hash,
            original_key,
        }) => {
            let original_entry = Entry::Agent(original_key.clone());
            let mut d = Delete::arbitrary(&mut ud).unwrap();
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::AgentPubKey;
            c.entry_hash = original_key.clone().into();
            d.deletes_address = original_action_hash;
            d.deletes_entry_address = original_key.clone().into();
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: Some(original_entry),
            })
        }
        OpType::RegisterDelete(OpDelete::PrivateEntry {
            original_action_hash,
            original_entry_hash,
            original_entry_type: et,
        }) => {
            let t = ScopedEntryDefIndex::try_from(&et).unwrap();
            let original_action = create(
                EntryVisibility::Private,
                &mut ud,
                t,
                original_entry_hash.clone(),
            );
            let original_action = EntryCreationAction::Create(original_action);
            let mut d = Delete::arbitrary(&mut ud).unwrap();
            d.deletes_address = original_action_hash;
            d.deletes_entry_address = original_entry_hash;
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterDelete(OpDelete::CapClaim {
            original_action_hash,
            original_entry_hash,
        }) => {
            let mut d = Delete::arbitrary(&mut ud).unwrap();
            d.deletes_address = original_action_hash;
            d.deletes_entry_address = original_entry_hash.clone();
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::CapClaim;
            c.entry_hash = original_entry_hash;
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterDelete(OpDelete::CapGrant {
            original_action_hash,
            original_entry_hash,
        }) => {
            let mut d = Delete::arbitrary(&mut ud).unwrap();
            d.deletes_address = original_action_hash;
            d.deletes_entry_address = original_entry_hash.clone();
            let mut c = Create::arbitrary(&mut ud).unwrap();
            c.entry_type = EntryType::CapGrant;
            c.entry_hash = original_entry_hash;
            let original_action = EntryCreationAction::Create(c);
            Op::RegisterDelete(RegisterDelete {
                delete: SignedHashed {
                    hashed: HoloHashed::from_content_sync(d),
                    signature: Signature::arbitrary(&mut ud).unwrap(),
                },
                original_action,
                original_entry: None,
            })
        }
        OpType::RegisterAgentActivity(activity) => {
            let r = match activity {
                OpActivity::CreateEntry {
                    entry_hash,
                    entry_type,
                } => activity_create(EntryVisibility::Public, &mut ud, entry_type, entry_hash),
                OpActivity::CreatePrivateEntry {
                    entry_hash,
                    entry_type,
                } => activity_create(EntryVisibility::Private, &mut ud, entry_type, entry_hash),
                OpActivity::CreateAgent{agent, ..} => {
                    let mut c = Create::arbitrary(&mut ud).unwrap();
                    c.entry_type = EntryType::AgentPubKey;
                    c.entry_hash = agent.into();
                    Action::Create(c)
                }
                OpActivity::UpdateEntry {
                    entry_hash,
                    original_action_hash,
                    original_entry_hash,
                    entry_type,
                } => {
                    let c = match activity_create(
                        EntryVisibility::Public,
                        &mut ud,
                        entry_type,
                        entry_hash,
                    ) {
                        Action::Create(c) => c,
                        _ => unreachable!(),
                    };
                    let mut u = Update::arbitrary(&mut ud).unwrap();
                    u.entry_hash = c.entry_hash;
                    u.original_action_address = original_action_hash;
                    u.original_entry_address = original_entry_hash;
                    u.entry_type = c.entry_type;
                    Action::Update(u)
                }
                OpActivity::UpdatePrivateEntry {
                    entry_hash,
                    original_action_hash,
                    original_entry_hash,
                    entry_type,
                } => {
                    let c = match activity_create(
                        EntryVisibility::Private,
                        &mut ud,
                        entry_type,
                        entry_hash,
                    ) {
                        Action::Create(c) => c,
                        _ => unreachable!(),
                    };
                    let mut u = Update::arbitrary(&mut ud).unwrap();
                    u.entry_hash = c.entry_hash;
                    u.original_action_address = original_action_hash;
                    u.original_entry_address = original_entry_hash;
                    u.entry_type = c.entry_type;
                    Action::Update(u)
                }
                OpActivity::UpdateAgent {
                    original_action_hash,
                    original_key,
                    new_key,
                } => {
                    let mut u = Update::arbitrary(&mut ud).unwrap();
                    u.entry_hash = new_key.into();
                    u.original_action_address = original_action_hash;
                    u.original_entry_address = original_key.into();
                    u.entry_type = EntryType::AgentPubKey;
                    Action::Update(u)
                }
                OpActivity::DeleteEntry {
                    original_action_hash,
                    original_entry_hash,
                } => {
                    let mut d = Delete::arbitrary(&mut ud).unwrap();
                    d.deletes_address = original_action_hash;
                    d.deletes_entry_address = original_entry_hash;
                    Action::Delete(d)
                }
                OpActivity::CreateLink {
                    base_address,
                    target_address,
                    tag,
                    link_type: lt,
                } => {
                    let mut c = CreateLink::arbitrary(&mut ud).unwrap();
                    c.base_address = base_address;
                    c.target_address = target_address;
                    c.tag = tag;
                    match lt {
                        Some(lt) => {
                            let t = ScopedLinkType::try_from(&lt).unwrap();
                            c.zome_index = t.zome_index;
                            c.link_type = t.zome_type;
                        }
                        None => {
                            c.zome_index = 200.into();
                            c.link_type = 0.into();
                        }
                    }
                    Action::CreateLink(c)
                }
                OpActivity::DeleteLink{original_action_hash: deletes} => {
                    let mut d = DeleteLink::arbitrary(&mut ud).unwrap();
                    d.link_add_address = deletes;
                    Action::DeleteLink(d)
                }
                OpActivity::CreateCapClaim{entry_hash, ..} => {
                    let mut c = Create::arbitrary(&mut ud).unwrap();
                    c.entry_hash = entry_hash;
                    c.entry_type = EntryType::CapClaim;
                    Action::Create(c)
                }
                OpActivity::CreateCapGrant{entry_hash, ..} => {
                    let mut c = Create::arbitrary(&mut ud).unwrap();
                    c.entry_hash = entry_hash;
                    c.entry_type = EntryType::CapGrant;
                    Action::Create(c)
                }
                OpActivity::UpdateCapClaim {
                    original_action_hash,
                    original_entry_hash,
                    action,
                } => {
                    let mut u = Update::arbitrary(&mut ud).unwrap();
                    u.entry_hash = action.entry_hash;
                    u.entry_type = EntryType::CapClaim;
                    u.original_action_address = original_action_hash;
                    u.original_entry_address = original_entry_hash;
                    Action::Update(u)
                }
                OpActivity::UpdateCapGrant {
                    original_action_hash,
                    original_entry_hash,
                    action,
                } => {
                    let mut u = Update::arbitrary(&mut ud).unwrap();
                    u.entry_hash = action.entry_hash;
                    u.entry_type = EntryType::CapGrant;
                    u.original_action_address = original_action_hash;
                    u.original_entry_address = original_entry_hash;
                    Action::Update(u)
                }
                OpActivity::Dna{dna_hash, ..} => {
                    let mut d = Dna::arbitrary(&mut ud).unwrap();
                    d.hash = dna_hash;
                    Action::Dna(d)
                }
                OpActivity::OpenChain{previous_dna_hash, ..} => {
                    let mut d = OpenChain::arbitrary(&mut ud).unwrap();
                    d.prev_dna_hash = previous_dna_hash;
                    Action::OpenChain(d)
                }
                OpActivity::CloseChain{new_dna_hash, ..} => {
                    let mut d = CloseChain::arbitrary(&mut ud).unwrap();
                    d.new_dna_hash = new_dna_hash;
                    Action::CloseChain(d)
                }
                OpActivity::AgentValidationPkg{membrane_proof, ..} => {
                    let mut d = AgentValidationPkg::arbitrary(&mut ud).unwrap();
                    d.membrane_proof = membrane_proof;
                    Action::AgentValidationPkg(d)
                }
                OpActivity::InitZomesComplete{..} => {
                    let d = InitZomesComplete::arbitrary(&mut ud).unwrap();
                    Action::InitZomesComplete(d)
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

fn activity_create<ET>(
    visibility: EntryVisibility,
    ud: &mut Unstructured,
    entry_type: Option<ET>,
    entry_hash: EntryHash,
) -> Action
where
    ScopedEntryDefIndex: for<'a> TryFrom<&'a ET, Error = WasmError>,
{
    let t = entry_type.map(|et| ScopedEntryDefIndex::try_from(&et).unwrap());
    let mut c = Create::arbitrary(ud).unwrap();
    c.entry_hash = entry_hash;
    match t {
        Some(t) => {
            c.entry_type = EntryType::App(AppEntryDef {
                entry_index: t.zome_type,
                zome_index: t.zome_index,
                visibility,
            })
        }
        None => {
            // Make sure this is out of range for this test.
            c.entry_type = EntryType::App(AppEntryDef {
                entry_index: 0.into(),
                zome_index: 200.into(),
                visibility,
            })
        }
    }
    Action::Create(c)
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
            OpRecord::CreatePrivateEntry { app_entry_type: _, .. } => unreachable!(),
            OpRecord::CreateAgent{..} => (),
            OpRecord::CreateCapClaim{..} => (),
            OpRecord::CreateCapGrant{..} => (),
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
            OpRecord::DeleteLink{..} => (),
            OpRecord::Dna{..} => (),
            OpRecord::OpenChain{..} => (),
            OpRecord::CloseChain{..} => (),
            OpRecord::AgentValidationPkg{..} => (),
            OpRecord::InitZomesComplete{..} => (),
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
