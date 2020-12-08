use crate::hdk3::prelude::*;

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfoOutput> {
    Ok(ZomeInfoOutput::new(crate::hdk3::prelude::zome_info()?))
}
