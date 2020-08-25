use hdk3::prelude::*;

#[hdk_extern]
fn sys_time(_: ()) -> ExternResult<SysTimeOutput> {
    Ok(SysTimeOutput::new(sys_time!()?))
}
