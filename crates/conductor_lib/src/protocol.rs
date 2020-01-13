use crate::conductor::CellHandle;
use async_trait::async_trait;
use crossbeam_channel::Sender;
use holochain_json_api::json::JsonString;
use lib3h_protocol::protocol_client::Lib3hClientProtocol;
use lib3h_protocol::protocol_server::Lib3hServerProtocol;
use skunkworx_core::cell::Cell;
use skunkworx_core::cell::CellApi;
use skunkworx_core::types::ZomeInvocation;
use skunkworx_core::types::ZomeInvocationResult;
use skunkworx_core_types::error::SkunkResult;

pub struct ConductorRequest<Response> {
    payload: ConductorProtocol,
    tx_response: Sender<Response>,
}

/// The set of messages that a conductor understands how to handle
pub enum ConductorProtocol {
    Admin(AdminMethod),
    Crypto(Crypto),
    Network(Lib3hServerProtocol),
    Test(Test),
    ZomeInvocation(CellHandle, ZomeInvocation),
}

pub enum AdminMethod {}

pub enum Crypto {
    Sign(String),
    Decrypt(String),
    Encrypt(String),
}

pub enum Test {
    AddAgent(AddAgentArgs),
}

pub struct AddAgentArgs {
    id: String,
    name: String,
}

#[async_trait]
pub trait ConductorApiExternal<Cell: CellApi> {
    async fn admin(method: AdminMethod) -> SkunkResult<JsonString>;

    async fn test(cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult;
}

#[async_trait]
pub trait ConductorApiInternal<Cell: CellApi> {
    async fn invoke_zome(cell: Cell, invocation: ZomeInvocation) -> ZomeInvocationResult;

    async fn net_send(message: Lib3hClientProtocol) -> SkunkResult<()>;

    async fn net_request(message: Lib3hClientProtocol) -> SkunkResult<Lib3hServerProtocol>;
}
