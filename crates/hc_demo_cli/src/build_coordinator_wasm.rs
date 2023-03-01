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
    let file_hash = create_entry(&EntryTypes::File(file.clone()))?;
    let record = get(file_hash.clone(), GetOptions::default())?
        .ok_or(
            wasm_error!(
                WasmErrorInner::Guest(String::from("Could not find the newly created File"))
            ),
        )?;
    let path = Path::from("all_files");
    create_link(path.path_entry_hash()?, file_hash.clone(), LinkTypes::AllFiles, ())?;
    Ok(record)
}

