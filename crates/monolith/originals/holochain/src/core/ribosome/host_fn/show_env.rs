use monolith::holochain::core::ribosome::error::RibosomeResult;
use monolith::holochain::core::ribosome::CallContext;
use monolith::holochain::core::ribosome::RibosomeT;
use monolith::holochain_zome_types::ShowEnvInput;
use monolith::holochain_zome_types::ShowEnvOutput;
use std::sync::Arc;

pub fn show_env(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: ShowEnvInput,
) -> RibosomeResult<ShowEnvOutput> {
    unimplemented!();
}
