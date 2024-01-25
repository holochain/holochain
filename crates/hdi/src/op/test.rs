use super::*;
use crate as hdi;
use crate::test_utils::set_zome_types;
use crate::test_utils::short_hand::*;
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

#[test_case(0, 100 => matches Err(WasmErrorInner::Guest(_)) ; "entry type is out of range")]
#[test_case(100, 0 => matches Ok(None) ; "zome id is out of range")]
#[test_case(0, 0 => matches Ok(Some(UnitEntryTypes::A)) ; "unit a")]
#[test_case(0, 1 => matches Ok(Some(UnitEntryTypes::B)) ; "unit b")]
#[test_case(0, 2 => matches Ok(Some(UnitEntryTypes::C)) ; "unit c")]
fn test_get_unit_entry_type(
    zome_index: u8,
    entry_type: u8,
) -> Result<Option<UnitEntryTypes>, WasmErrorInner> {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    get_unit_entry_type::<EntryTypes>(zome_index.into(), entry_type.into()).map_err(|e| e.error)
}

#[test_case(
    EntryType::App(public_app_entry_def(0, 0))
    => matches Ok(ActivityEntry::App{entry_type: Some(UnitEntryTypes::A), ..}) ; "unit a")]
#[test_case(
    EntryType::App(public_app_entry_def(0, 1))
    => matches Ok(ActivityEntry::App{entry_type: Some(UnitEntryTypes::B), ..}) ; "unit b")]
#[test_case(
    EntryType::App(public_app_entry_def(0, 2))
    => matches Ok(ActivityEntry::App{entry_type: Some(UnitEntryTypes::C), ..}) ; "unit c")]
#[test_case(
    EntryType::App(private_app_entry_def(0, 0))
    => matches Ok(ActivityEntry::PrivateApp{entry_type: Some(UnitEntryTypes::A), ..}) ; "private unit a")]
#[test_case(
    EntryType::App(private_app_entry_def(0, 1))
    => matches Ok(ActivityEntry::PrivateApp{entry_type: Some(UnitEntryTypes::B), ..}) ; "private unit b")]
#[test_case(
    EntryType::App(private_app_entry_def(0, 2))
    => matches Ok(ActivityEntry::PrivateApp{entry_type: Some(UnitEntryTypes::C), ..}) ; "private unit c")]
#[test_case(EntryType::AgentPubKey => matches Ok(ActivityEntry::Agent(_)); "agent")]
#[test_case(EntryType::CapClaim => matches Ok(ActivityEntry::CapClaim(_)); "cap claim")]
#[test_case(EntryType::CapGrant => matches Ok(ActivityEntry::CapGrant(_)); "cap grant")]
#[test_case(EntryType::App(public_app_entry_def(0, 3)) => matches Err(WasmErrorInner::Guest(_)) ; "entry type out of range")]
#[test_case(EntryType::App(private_app_entry_def(0, 3)) => matches Err(WasmErrorInner::Guest(_)) ; "private entry type out of range")]
#[test_case(
    EntryType::App(public_app_entry_def(1, 0))
    => matches Ok(ActivityEntry::App{entry_type: None, ..}) ; "zome out of range")]
#[test_case(
    EntryType::App(private_app_entry_def(1, 0))
    => matches Ok(ActivityEntry::PrivateApp{entry_type: None, ..}) ; "private entry, zome out of range")]
fn test_activity_entry(
    entry_type: EntryType,
) -> Result<ActivityEntry<UnitEntryTypes>, WasmErrorInner> {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    activity_entry::<EntryTypes>(&entry_type, &eh(0)).map_err(|e| e.error)
}

#[test_case(
    public_app_entry_def(0, 0), Some(&e(A{}))
    => matches Ok(UnitEnumEither::Enum(EntryTypes::A(A{}))) ; "a")]
#[test_case(
    public_app_entry_def(0, 1), Some(&e(B{}))
    => matches Ok(UnitEnumEither::Enum(EntryTypes::B(B{}))) ; "b")]
#[test_case(
    public_app_entry_def(0, 2), Some(&e(C{}))
    => matches Ok(UnitEnumEither::Enum(EntryTypes::C(C{}))) ; "c")]
#[test_case(
    private_app_entry_def(0, 0), None
    => matches Ok(UnitEnumEither::Unit(UnitEntryTypes::A)) ; "private a")]
#[test_case(
    private_app_entry_def(0, 1), None
    => matches Ok(UnitEnumEither::Unit(UnitEntryTypes::B)) ; "private b")]
#[test_case(
    private_app_entry_def(0, 2), None
    => matches Ok(UnitEnumEither::Unit(UnitEntryTypes::C)) ; "private c")]
#[test_case(
    public_app_entry_def(0, 0), Some(&e(D::default()))
    => matches Err(WasmErrorInner::Serialize(_)) ; "deserialization failure")]
#[test_case(
    public_app_entry_def(0, 3), Some(&e(A{}))
    => matches Err(WasmErrorInner::Guest(_)) ; "entry type out of range")]
#[test_case(
    private_app_entry_def(0, 3), None
    => matches Err(WasmErrorInner::Guest(_)) ; "private entry type out of range")]
#[test_case(
    public_app_entry_def(1, 0), Some(&e(A{}))
    => matches Err(WasmErrorInner::Host(_)) ; "zome id out of range")]
#[test_case(
    private_app_entry_def(1, 0), None
    => matches Err(WasmErrorInner::Host(_)) ; "private entry zome id out of range")]
#[test_case(
    public_app_entry_def(0, 0), None
    => matches Err(WasmErrorInner::Guest(_)) ; "public entry missing")]
#[test_case(
    private_app_entry_def(0, 0), Some(&e(A{}))
    => matches Err(WasmErrorInner::Guest(_)) ; "private entry present")]
fn test_get_app_entry_type_for_record_authority(
    entry_type: AppEntryDef,
    entry: Option<&Entry>,
) -> Result<UnitEnumEither<EntryTypes>, WasmErrorInner> {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    get_app_entry_type_for_record_authority::<EntryTypes>(&entry_type, entry).map_err(|e| e.error)
}

#[test_case(0, 0 => matches Ok(LinkTypes::A) ; "a")]
#[test_case(0, 1 => matches Ok(LinkTypes::B) ; "b")]
#[test_case(0, 2 => matches Ok(LinkTypes::C) ; "c")]
#[test_case(0, 3 => matches Err(WasmErrorInner::Guest(_)) ; "link type out of scope")]
#[test_case(1, 0 => matches Err(WasmErrorInner::Host(_)) ; "zome out of scope")]
fn test_in_scope_link_type(zome_index: u8, link_type: u8) -> Result<LinkTypes, WasmErrorInner> {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    in_scope_link_type::<LinkTypes>(zome_index.into(), link_type.into()).map_err(|e| e.error)
}

#[test_case(0, 0 => matches Ok(Some(LinkTypes::A)) ; "a")]
#[test_case(0, 1 => matches Ok(Some(LinkTypes::B)) ; "b")]
#[test_case(0, 2 => matches Ok(Some(LinkTypes::C)) ; "c")]
#[test_case(0, 3 => matches Err(WasmErrorInner::Guest(_)); "link type out of scope")]
#[test_case(1, 0 => matches Ok(None) ; "zome out of scope is none")]
fn test_activity_link_type(
    zome_index: u8,
    link_type: u8,
) -> Result<Option<LinkTypes>, WasmErrorInner> {
    set_zome_types(&[(0, 3)], &[(0, 3)]);
    activity_link_type::<LinkTypes>(zome_index.into(), link_type.into()).map_err(|e| e.error)
}
