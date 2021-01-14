use hdk3::prelude::*;

#[hdk_extern]
fn zome_info(_: ()) -> ExternResult<ZomeInfo> {
    hdk3::prelude::zome_info()
}
