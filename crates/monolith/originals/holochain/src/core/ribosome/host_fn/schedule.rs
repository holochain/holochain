use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::ScheduleInput;
use monolith::holochain_zome_types::ScheduleOutput;
use std::sync::Arc;

pub fn schedule(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: ScheduleInput,
) -> RibosomeResult<ScheduleOutput> {
    unimplemented!()
}
