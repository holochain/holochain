use hdi::prelude::*;

#[dna_properties]
pub struct MyProperties {
    authority_agent: Vec<u8>,
    max_count: u32,
    contract_address: String
}

#[hdk_extern]
pub fn get_dna_properties(_: ()) -> ExternResult<MyProperties> {
    MyProperties::try_from_dna_properties()
}
