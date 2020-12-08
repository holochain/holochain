use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::ScheduleInput;
use crate::holochain_zome_types::ScheduleOutput;
use std::sync::Arc;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: ScheduleInput,
) -> RibosomeResult<ScheduleOutput> {
    unimplemented!()
}
