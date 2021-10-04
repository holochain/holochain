use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::WasmError;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;
use futures::future::join_all;

#[allow(clippy::extra_unused_lifetimes)]
pub fn get<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetInput>,
) -> Result<Vec<Option<Element>>, WasmError> {
    match HostFnAccess::from(call_context.host_context()) {
        HostFnAccess{ read_workspace: Permission::Allow, .. } => {
            let results: Vec<Result<Option<Element>, _>> = tokio_helper::block_forever_on(async move {
                join_all(inputs.into_iter().map(|input| {
                    async {
                        let GetInput {
                            any_dht_hash,
                            get_options,
                        } = input;
                        Cascade::from_workspace_network(
                            call_context.host_context.workspace(),
                            call_context.host_context.network().clone()
                        )
                        .dht_get(any_dht_hash, get_options).await
                    }
                })).await
            });
            let results: Result<Vec<_>, _> = results.into_iter().map(|result| match result {
                Ok(v) => Ok(v),
                Err(cascade_error) => Err(WasmError::Host(cascade_error.to_string())),
            }).collect();
            Ok(results?)
        },
        _ => unreachable!("tried to call `get` in a context where it is not permitted to be called"),
    }
}

// we are relying on the create tests to show the commit/get round trip
// See create.rs
