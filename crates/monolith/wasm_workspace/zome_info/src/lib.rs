use hdk3::prelude::*;

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfoOutput> {
    Ok(ZomeInfoOutput::new(hdk3::prelude::zome_info()?))
}
