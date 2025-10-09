use crate::core::ribosome::CallContext;
use crate::core::ribosome::HostFnAccess;
use crate::core::ribosome::RibosomeError;
use crate::core::ribosome::RibosomeT;
use holochain_cascade::CascadeImpl;
use holochain_p2p::actor::GetActivityOptions;
use holochain_state::prelude::insert_op_dht;
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
            write_workspace: Permission::Allow,
            ..
        } => {
            let GetAgentActivityInput {
                agent_pubkey,
                chain_query_filter,
                activity_request,
            } = input;
            let options = match activity_request {
                ActivityRequest::Status => GetActivityOptions {
                    include_valid_activity: false,
                    include_rejected_activity: false,
                    get_options: GetOptions::local(),
                    ..Default::default()
                },
                ActivityRequest::Full => GetActivityOptions {
                    include_valid_activity: true,
                    include_rejected_activity: true,
                    get_options: GetOptions::local(),
                    ..Default::default()
                },
            };

            // Get the network from the context
            let network = call_context.host_context.network().clone();

            // timeouts must be handled by the network
            tokio_helper::block_forever_on(async move {
                let workspace = call_context.host_context.workspace();
                let cascade = CascadeImpl::from_workspace_and_network(&workspace, network);
                let activity = cascade
                    .get_agent_activity(agent_pubkey, chain_query_filter, options)
                    .await
                    .map_err(|cascade_error| {
                        wasm_error!(WasmErrorInner::Host(cascade_error.to_string()))
                    })?;
                println!("hello activity {activity:?}");
                if !activity.warrants.is_empty() {
                    let warrant_op = WarrantOp::from(activity.warrants[0].clone());

                    let dht_op = DhtOp::WarrantOp(Box::new(warrant_op));
                    let dht_op_hashed = DhtOpHashed::from_content_sync(dht_op);

                    let dht_db = call_context
                        .host_context
                        .workspace_write()
                        .source_chain()
                        .as_ref()
                        .expect("Must have source chain if write_workspace access is given")
                        .dht_db()
                        .clone();

                    dht_db
                        .write_async(move |txn| insert_op_dht(txn, &dht_op_hashed, 0, None))
                        .await
                        .map_err(|e| wasm_error!(WasmErrorInner::Host(e.to_string())))?;

                    // match InclusiveTimestampInterval::try_new(Timestamp::now(), Timestamp::max()) {
                    //     Ok(interval) => {
                    //         // All of these warrants must be issued against the same warrantee.
                    //         // Hence use the first warrant to block the warrantee.
                    //         let cell_id = CellId::new(
                    //             call_context.host_context.network().dna_hash().clone(),
                    //             activity.warrants[0].warrantee.clone(),
                    //         );
                    //         if let Err(err) = call_context
                    //             .host_context
                    //             .network()
                    //             .block(Block::new(
                    //                 BlockTarget::Cell(cell_id, CellBlockReason::InvalOp),
                    //                 interval,
                    //             ))
                    //             .await
                    //         {
                    //             tracing::warn!(
                    //                 ?err,
                    //                 "error blocking agent after receiving warrant"
                    //             );
                    //         }
                    //     }
                    //     Err(err) => {
                    //         tracing::error!(?err, "invalid interval when blocking an agent");
                    //     }
                    // }
                }

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
