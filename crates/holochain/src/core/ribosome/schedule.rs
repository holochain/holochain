use super::HostContext;
use super::WasmRibosome;
use crate::core::ribosome::error::RibosomeResult;
use holochain_zome_types::ScheduleInput;
use holochain_zome_types::ScheduleOutput;
use std::sync::Arc;

pub async fn schedule(
    _ribosome: Arc<WasmRibosome>,
    _host_context: Arc<HostContext>,
    _input: ScheduleInput,
) -> RibosomeResult<ScheduleOutput> {
    unimplemented!()
}
