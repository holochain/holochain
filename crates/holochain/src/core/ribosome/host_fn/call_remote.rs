use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_wasmer_host::prelude::SerializedBytes;
use holochain_zome_types::CallRemoteInput;
use holochain_zome_types::CallRemoteOutput;
use std::sync::Arc;

const CALL_REMOTE_TIMEOUT: u64 = 10_000;

pub fn call_remote(
    _ribosome: Arc<WasmRibosome>,
    call_context: Arc<CallContext>,
    input: CallRemoteInput,
) -> RibosomeResult<CallRemoteOutput> {
    let result: SerializedBytes = tokio_safe_block_on::tokio_safe_block_on(
        async move {
            let mut network = call_context.host_access().network().clone();
            let call_remote = input.into_inner();
            network
                .call_remote(
                    call_remote.to_agent(),
                    call_remote.zome_name(),
                    call_remote.fn_name(),
                    call_remote.cap(),
                    call_remote.request(),
                )
                .await
        },
        std::time::Duration::from_millis(CALL_REMOTE_TIMEOUT),
    )??;

    Ok(CallRemoteOutput::new(result))
}
