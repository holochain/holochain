#[macro_use]
extern crate serde_derive;

mod opaque;

use async_trait::async_trait;
use opaque::Opaque;
use sx_types::shims::*;

type SpaceHash = String;
type EntryHash = String;

#[async_trait]
trait ClientRequestProtocol {
    async fn join_space();
    async fn leave_space();
    async fn send_direct_message();
    async fn fetch_entry();
    async fn publish_entry();
    async fn query_entry(data: QueryEntryRequest) -> QueryEntryResponse;
}

#[async_trait]
trait ClientHandlerProtocol {
    fn handle_send_direct_message();
    fn handle_fetch_entry_result();
    fn handle_store_transform();
    fn handle_drop_entry();
    fn handle_get_authoring_list();
    fn handle_get_gossiping_list();
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QueryEntryRequest {
    pub space_address: SpaceHash,
    pub entry_address: EntryHash,
    pub request_id: String,
    pub requester_agent_id: AgentPubKey,
    pub query: Opaque,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QueryEntryResponse {
    pub space_address: SpaceHash,
    pub entry_address: EntryHash,
    pub request_id: String,
    pub requester_agent_id: AgentPubKey,
    pub responder_agent_id: AgentPubKey,
    pub query_result: Opaque,
}
