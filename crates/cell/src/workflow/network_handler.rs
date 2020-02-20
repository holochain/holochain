use async_trait::async_trait;
use sx_types::{error::SkunkResult, shims::*};

pub async fn handle_network_message(
    msg: Lib3hToClient,
) -> SkunkResult<Option<Lib3hToClientResponse>> {
    match msg {
        _ => Ok(Some(Lib3hToClientResponse)),
    }
}

#[async_trait]
trait NetworkMessageHandlerT {
    //     async fn handle_send_direct_message(data: DirectMessageData) -> SkunkResult<ZomeInvocationResult>;
    async fn handle_store_dht_transform(transform: DhtItem) -> SkunkResult<()>;
    //     async fn handle_query_entry(requester: AgentId, query: QueryData) -> SkunkResult<QueryEntryResultData>;
}

pub struct NetworkMessageHandler;

#[async_trait]
impl NetworkMessageHandlerT for NetworkMessageHandler {
    async fn handle_store_dht_transform(_transform: DhtItem) -> SkunkResult<()> {
        Ok(())
    }
}

struct DhtItem;
// struct QueryData;
