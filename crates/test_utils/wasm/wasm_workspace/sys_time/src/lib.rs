use hdk3::prelude::*;

#[hdk_extern]
fn sys_time(_: ()) -> ExternResult<core::time::Duration> {
    hdk3::prelude::sys_time()
}
