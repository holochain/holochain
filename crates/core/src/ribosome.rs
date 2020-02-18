use sx_types::entry::Entry;
use crate::{
    agent::{SourceChainCommitBundle, SourceChain},
    nucleus::{ZomeInvocation, ZomeInvocationResult},
    txn::source_chain,
    wasm_engine::WasmEngine,
};
use sx_types::{dna::Dna, error::SkunkResult, shims::*};

pub trait RibosomeT {
    fn run_validation(self, cursor: &source_chain::Cursor, entry: Entry) -> ValidationResult;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        self,
        bundle: SourceChainCommitBundle,
        invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<(ZomeInvocationResult, SourceChainCommitBundle)>;
}

/// TODO determine what cursor looks like for ribosomes
/// Total hack just to have something to look at
/// The only Ribosome is a Wasm ribosome.
pub struct Ribosome {
    engine: WasmEngine,
}

impl Ribosome {
    pub fn new(dna: Dna) -> Self {
        Self { engine: WasmEngine }
    }
}

impl RibosomeT for Ribosome {
    fn run_validation(self, cursor: &source_chain::Cursor, entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function(
        self,
        bundle: SourceChainCommitBundle,
        invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<(ZomeInvocationResult, SourceChainCommitBundle)> {
        unimplemented!()
    }
}
