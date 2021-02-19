use hdk::prelude::*;

#[hdk_extern]
fn sys_time(_: ()) -> ExternResult<core::time::Duration> {
    hdk::prelude::sys_time()
}
