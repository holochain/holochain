#[cfg(feature = "build_demo")]
compile_error!("feature build_demo is incompatible with build_coordinator_wasm");

#[cfg(feature = "build_integrity_wasm")]
compile_error!("feature build_integrity_wasm is incompatible with build_coordinator_wasm");

/// One crate can build a demo or integrity or coordinator wasm
pub const BUILD_MODE: &str = "build_coordinator_wasm";

use hdk::prelude::*;

super::wasm_common!();

#[hdk_extern]
pub fn init(_: ()) -> ExternResult<InitCallbackResult> {
    Ok(InitCallbackResult::Pass)
}

#[hdk_extern]
pub fn create_file(file: File) -> ExternResult<Record> {
    let file_hash = create_entry(&EntryTypes::File(file))?;
    let record = get(file_hash.clone(), GetOptions::default())?.ok_or(wasm_error!(
        WasmErrorInner::Guest(String::from("Could not find the newly created File"))
    ))?;
    let path = Path::from("all_files");
    create_link(path.path_entry_hash()?, file_hash, LinkTypes::AllFiles, ())?;
    Ok(record)
}

#[hdk_extern]
pub fn get_file(hash: ActionHash) -> ExternResult<Option<File>> {
    let record: Record = match get(hash, GetOptions::default())? {
        Some(r) => r,
        None => return Ok(None),
    };

    record.entry.to_app_option().map_err(|e| wasm_error!(e))
}

#[hdk_extern]
pub fn get_all_files(_: ()) -> ExternResult<Vec<ActionHash>> {
    let path = Path::from("all_files");
    let links = get_links(path.path_entry_hash()?, LinkTypes::AllFiles, None)?;
    let get_input: Vec<GetInput> = links
        .into_iter()
        .map(|link| GetInput::new(ActionHash::from(link.target).into(), GetOptions::default()))
        .collect();
    let records = HDK.with(|hdk| hdk.borrow().get(get_input))?;
    let hashes: Vec<ActionHash> = records
        .into_iter()
        .flatten()
        .map(|r| r.action_address().clone())
        .collect();
    Ok(hashes)
}
