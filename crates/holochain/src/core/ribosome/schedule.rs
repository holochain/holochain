use super::HostContext;
use super::WasmRibosome;
use std::sync::Arc;
use sx_zome_types::ScheduleInput;
use sx_zome_types::ScheduleOutput;

pub fn schedule(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: ScheduleInput,
) -> ScheduleOutput {
    unimplemented!()
}
