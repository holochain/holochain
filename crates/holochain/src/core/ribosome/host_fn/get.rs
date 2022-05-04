use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use futures::StreamExt;
use holochain_cascade::Cascade;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;

#[allow(clippy::extra_unused_lifetimes)]
#[tracing::instrument(skip(_ribosome, call_context), fields(?call_context.zome, function = ?call_context.function_name))]
pub fn get<'a>(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    inputs: Vec<GetInput>,
) -> Result<Vec<Option<Element>>, RuntimeError> {
    let num_requests = inputs.len();
    tracing::debug!("Starting with {} requests.", num_requests);
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let results: Vec<Result<Option<Element>, _>> =
                tokio_helper::block_forever_on(async move {
                    futures::stream::iter(inputs.into_iter().map(|input| async {
                        let GetInput {
                            any_dht_hash,
                            get_options,
                        } = input;
                        Cascade::from_workspace_network(
                            &call_context.host_context.workspace(),
                            call_context.host_context.network().clone(),
                        )
                        .dht_get(any_dht_hash, get_options)
                        .await
                    }))
                    // Limit concurrent calls to 10 as each call
                    // can spawn multiple connections.
                    .buffered(10)
                    .collect()
                    .await
                });
            let results: Result<Vec<_>, _> = results
                .into_iter()
                .map(|result| match result {
                    Ok(v) => Ok(v),
                    Err(cascade_error) => Err(wasm_error!(WasmErrorInner::Host(cascade_error.to_string()))),
                })
                .collect();
            let results = results?;
            tracing::debug!(
                "Ending with {} out of {} results and {} total responses.",
                results.iter().filter(|r| r.is_some()).count(),
                num_requests,
                results.len(),
            );
            Ok(results)
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get".into(),
            )
            .to_string(),
        ))),
    }
}

// we are relying on the create tests to show the commit/get round trip
// See create.rs
