use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::wasm_ribosome::WasmRibosome;
use crate::core::ribosome::CallContext;
use holochain_wasmer_host::prelude::SerializedBytes;
use holochain_zome_types::RemoteCallInput;
use holochain_zome_types::RemoteCallOutput;
use std::sync::Arc;

const REMOTE_CALL_TIMEOUT: u64 = 10_000;

pub fn remote_call(
    _ribosome: Arc<WasmRibosome>,
    call_context: Arc<CallContext>,
    input: RemoteCallInput,
) -> RibosomeResult<RemoteCallOutput> {
    let result: SerializedBytes = tokio_safe_block_on::tokio_safe_block_on(
        async move {
            let mut network = call_context.host_access().network().clone();
            let remote_call = input.into_inner();
            network
                .call_remote(
                    remote_call.to_agent(),
                    remote_call.zome_name(),
                    remote_call.fn_name(),
                    remote_call.cap(),
                    remote_call.request(),
                )
                .await
        },
        std::time::Duration::from_millis(REMOTE_CALL_TIMEOUT),
    )??;

    Ok(RemoteCallOutput::new(result))
}
