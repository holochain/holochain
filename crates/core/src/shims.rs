use crate::agent::SourceChain;
use crate::error::SkunkResult;
use crate::types::cursor::CursorR;
use crate::types::cursor::CursorRw;
use crate::types::ZomeInvocation;
use crate::types::ZomeInvocationResult;

pub type Address = String;
pub trait AddressableContent {}
#[derive(Clone, PartialEq, Hash, Eq)]
pub struct AgentId;
pub struct CapToken;
pub struct CapabilityRequest;
pub struct DhtTransform;
pub struct Dna;
pub struct Entry;
pub type JsonString = String;
pub type Content = JsonString;

pub struct Lib3hToClient;
pub struct Lib3hToClientResponse;
pub struct Lib3hClientProtocol;
pub struct Lib3hToServer;
pub struct Lib3hToServerResponse;
pub struct Lib3hServerProtocol;

pub struct PersistenceError;
pub type PersistenceResult<T> = Result<T, PersistenceError>;

pub enum ValidationResult {
    Valid,
    Invalid,
    Pending,
}

/// Total hack just to have something to look at
pub struct Ribosome;
impl Ribosome {
    pub fn new(dna: Dna) -> Self {
        Self
    }

    pub fn run_validation<C: CursorR>(self, cursor: &C, entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    pub fn call_zome_function<C: CursorRw>(
        self,
        cursor: C,
        invocation: ZomeInvocation,
        source_chain: SourceChain,
    ) -> SkunkResult<(ZomeInvocationResult, C)> {
        unimplemented!()
    }
}
