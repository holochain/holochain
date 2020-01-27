use crate::{
    error::Lib3hProtocolResult, protocol_client::Lib3hClientProtocol,
    protocol_server::Lib3hServerProtocol, uri::Lib3hUri, DidWork,
};

/// Common interface for all types of network modules to be used by the Lib3hWorker
pub trait NetworkEngine {
    /// Post a protocol message from core -> lib3h
    fn post(&mut self, data: Lib3hClientProtocol) -> Lib3hProtocolResult<()>;
    /// A single iteration of the networking process loop
    /// (should be called frequently to not cpu starve networking)
    /// Returns a vector of protocol messages core is required to handle.
    fn process(&mut self) -> Lib3hProtocolResult<(DidWork, Vec<Lib3hServerProtocol>)>;
    /// Get qualified transport address
    fn advertise(&self) -> Lib3hUri;

    /// User supplied engine identifier
    fn name(&self) -> String;
}
