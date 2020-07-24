use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_zome_types::ScheduleInput;
use holochain_zome_types::ScheduleOutput;
use std::sync::Arc;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: ScheduleInput,
) -> RibosomeResult<ScheduleOutput> {
    unimplemented!()
}
