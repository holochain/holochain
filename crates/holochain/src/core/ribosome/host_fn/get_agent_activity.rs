use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::Cascade;
use holochain_p2p::actor::GetActivityOptions;
use holochain_types::prelude::*;
use holochain_wasmer_host::prelude::*;
use std::sync::Arc;
use crate::core::ribosome::HostFnAccess;

pub fn get_agent_activity(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: GetAgentActivityInput,
) -> Result<AgentActivity, RuntimeError> {
    match HostFnAccess::from(&call_context.host_context()) {
        HostFnAccess{ read_workspace: Permission::Allow, .. } => {
            let GetAgentActivityInput {
                agent_pubkey,
                chain_query_filter,
                activity_request,
            } = input;
            let options = match activity_request {
                ActivityRequest::Status => GetActivityOptions {
                    include_valid_activity: false,
                    include_rejected_activity: false,
                    ..Default::default()
                },
                ActivityRequest::Full => GetActivityOptions {
                    include_valid_activity: true,
                    include_rejected_activity: true,
                    ..Default::default()
                },
            };

        // Get the network from the context
        let network = call_context.host_context.network().clone();

        // timeouts must be handled by the network
        tokio_helper::block_forever_on(async move {
            let workspace = call_context.host_context.workspace();
            let mut cascade = Cascade::from_workspace_network(&workspace, network);
            let activity = cascade
                .get_agent_activity(agent_pubkey, chain_query_filter, options)
                .await
                .map_err(|cascade_error| wasm_error!(WasmErrorInner::Host(cascade_error.to_string())))?;

                    Ok(activity.into())
                })
        },
        _ => Err(wasm_error!(WasmErrorInner::Host(RibosomeError::HostFnPermissions(
            call_context.zome.zome_name().clone(),
            call_context.function_name().clone(),
            "get_agent_activity".into()
        ).to_string())).into())
    }
}

// we are relying on the create tests to show the commit/get round trip
// See commit_entry.rs
