use hdi::prelude::*;
use holochain_test_wasm_common::MyValidDnaProperties;

#[hdk_extern]
pub fn get_dna_properties(_: ()) -> ExternResult<MyValidDnaProperties> {
    MyValidDnaProperties::try_from_dna_properties()
}
