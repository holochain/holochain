use holochain_wasmer_guest::*;
use holochain_zome_types::*;
use hdk3::prelude::*;

holochain_wasmer_guest::holochain_externs!();

map_extern!(validate_link, _validate_link);

pub fn _validate_link(_: ValidateLinkAddData) -> Result<ValidateLinkAddCallbackResult, WasmError> {
    Ok(ValidateLinkAddCallbackResult::Invalid("esoteric edge case (link version)".into()))
}
