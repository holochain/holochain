use skunkworx_core_types::agent::AgentId;
use crate::shims::{Lib3hClientProtocol, Lib3hServerProtocol};
use crate::types::ZomeInvocationResult;
use crate::{cell::Cell, types::ZomeInvocation};
use async_trait::async_trait;
use crossbeam_channel::Sender;
use futures::never::Never;
use lib3h_protocol::data_types::*;
use lib3h_protocol::protocol::*;
use skunkworx_core_types::error::SkunkResult;

pub async fn handle_network_message(
    msg: Lib3hToClient,
) -> SkunkResult<Option<Lib3hToClientResponse>> {
    match msg {
        _ => Ok(Some(Lib3hToClientResponse::HandleDropEntryResult))
    }
}

// #[async_trait]
// trait HandleNetworkMessage {
//     async fn handle_send_direct_message(data: DirectMessageData) -> SkunkResult<ZomeInvocationResult>;
//     async fn handle_store_dht_transform(transform: DhtItem) -> SkunkResult<()>;
//     async fn handle_query_entry(requester: AgentId, query: QueryData) -> SkunkResult<QueryEntryResultData>;
// }

// struct DhtItem;
// struct QueryData;