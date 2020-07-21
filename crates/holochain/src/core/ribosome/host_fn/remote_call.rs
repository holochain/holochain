use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_zome_types::RemoteCallInput;
use holochain_zome_types::RemoteCallOutput;
use holochain_wasmer_host::prelude::SerializedBytes;
use std::sync::Arc;

const REMOTE_CALL_TIMEOUT: u32 = 10_000;

pub fn remote_call(
    _ribosome: Arc<WasmRibosome>,
    call_context: Arc<CallContext>,
    input: RemoteCallInput,
) -> RibosomeResult<RemoteCallOutput> {

    let result: SerializedBytes = tokio_safe_block_on::tokio_safe_block_on(
        async move {
            let network = call_context.host_access().network();
            network.remote_call(
                input.to_agent(),
                input.zome_name(),
                input.fn_name(),
                input.cap(),
                input.request(),
            ).await
        },
        std::time::Duration::from_millis(REMOTE_CALL_TIMEOUT),
    )??;

    Ok(RemoteCallOutput::new(result))
}
