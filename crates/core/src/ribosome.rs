use crate::agent::SourceChain;
use crate::nucleus::ZomeInvocation;
use crate::nucleus::ZomeInvocationResult;
use crate::txn::source_chain;
use crate::wasm_engine::WasmEngine;
use sx_types::error::SkunkResult;
use sx_types::dna::Dna;
use sx_types::shims::*;

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

    pub fn run_validation(self, cursor: &source_chain::Cursor, entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    pub fn call_zome_function(
        self,
        cursor: source_chain::CursorRw,
        invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<(ZomeInvocationResult, source_chain::CursorRw)> {
        unimplemented!()
    }
}
