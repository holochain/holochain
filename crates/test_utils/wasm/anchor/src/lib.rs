use hdk3::prelude::*;

holochain_externs!();

#[no_mangle]
pub extern "C" fn entry_defs(_: GuestPtr) -> GuestPtr {
    let globals: ZomeGlobals = try_result!(host_call!(__globals, ()), "failed to get globals");

    let defs: EntryDefs = vec![
        Anchor::entry_def()
    ].into();

    ret!(GuestOutput::new(try_result!(EntryDefsCallbackResult::Defs(
        globals.zome_name,
        defs,
    ).try_into(), "failed to serialize entry defs")));
}


#[test]
fn anchor_namespace() {
    assert_eq!(
        hdk3::anchor::ROOT,
        "holochain_anchors::root",
    );

    assert_eq!(
        hdk3::anchor::ANCHOR,
        "holochain_anchors::anchor",
    );

    assert_eq!(
        hdk3::anchor::LINK,
        "holochain_anchors::link",
    );
}

#[test]
fn anchor_required_validations() {
    assert_eq!(
        hdk3::anchor::REQUIRED_VALIDATIONS,
        13,
    );
}
