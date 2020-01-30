use crate::agent::SourceChain;
use crate::agent::SourceChainSnapshot;
use crate::cursor::ChainCursorX;
use crate::cursor::SourceChainAttribute;
use crate::cursor::{CursorR, CursorRw};
use crate::types::ZomeInvocation;
use crate::types::ZomeInvocationResult;
use crate::wasm_engine::WasmEngine;
use sx_types::dna::Dna;
use sx_types::error::SkunkResult;
use sx_types::shims::*;

// trait RibosomeT {
//     fn run_validation<C: CursorR>(self, cursor: &C, entry: Entry) -> ValidationResult;
//     fn invoke_zome<C: CursorRw>(
//         self,
//         cursor: C,
//         invocation: ZomeInvocation,
//         source_chain: SourceChain,
//     ) -> SkunkResult<(ZomeInvocationResult, C)>;
// }

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

impl Ribosome {
    pub fn run_validation<C: CursorR<SourceChainAttribute>>(
        self,
        cursor: &C,
        entry: Entry,
    ) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    pub fn invoke_zome(
        self,
        cursor: ChainCursorX,
        invocation: ZomeInvocation,
        chain: SourceChainSnapshot,
    ) -> SkunkResult<(ZomeInvocationResult, ChainCursorX)> {
        unimplemented!()
    }
}
