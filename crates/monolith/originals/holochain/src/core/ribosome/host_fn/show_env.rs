use crate::holochain::core::ribosome::error::RibosomeResult;
use crate::holochain::core::ribosome::CallContext;
use crate::holochain::core::ribosome::RibosomeT;
use crate::holochain_zome_types::ShowEnvInput;
use crate::holochain_zome_types::ShowEnvOutput;
use std::sync::Arc;

pub fn show_env(
    _ribosome: Arc<impl RibosomeT>,
    _call_context: Arc<CallContext>,
    _input: ShowEnvInput,
) -> RibosomeResult<ShowEnvOutput> {
    unimplemented!();
}
