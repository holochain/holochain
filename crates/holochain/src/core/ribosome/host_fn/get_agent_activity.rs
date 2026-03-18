use crate::core::ribosome::host_fn::cascade_from_call_context;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::get_options_ext::GetOptionsExt;
use holochain_p2p::actor::GetActivityOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use wasmer::RuntimeError;

pub fn get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetAgentActivityInput,
) -> Result<AgentActivity, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess {
            read_workspace: Permission::Allow,
            ..
        } => {
            let GetAgentActivityInput {
                agent_pubkey,
                chain_query_filter,
                activity_request,
                get_options,
            } = input;
            let network_req_options = get_options.to_network_options();
            let options = match activity_request {
                ActivityRequest::Status => GetActivityOptions {
                    include_valid_activity: false,
                    include_rejected_activity: false,
                    get_options,
                    network_req_options,
                    ..Default::default()
                },
                ActivityRequest::Full => GetActivityOptions {
                    include_valid_activity: true,
                    include_rejected_activity: true,
                    get_options,
                    network_req_options,
                    ..Default::default()
                },
            };

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let cascade = cascade_from_call_context(&call_context);
                let activity = cascade
                    .get_agent_activity(agent_pubkey, chain_query_filter, options)
                    .await
                    .map_err(|cascade_error| {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string()))
                    })?;

                Ok(activity.into())
            })
        }
        _ => Err(wasm_error!(WasmErrorInner::Host(
            RibosomeError::HostFnPermissions(
                call_context.zome.zome_name().clone(),
                call_context.function_name().clone(),
                "get_agent_activity".into()
            )
            .to_string()
        ))
        .into()),
    }
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs
