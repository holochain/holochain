use crate::hdk3::prelude::*;

#[hdk_extern]
fn sys_time(_: ()) -> ExternResult<SysTimeOutput> {
    Ok(SysTimeOutput::new(crate::hdk3::prelude::sys_time()?))
}
