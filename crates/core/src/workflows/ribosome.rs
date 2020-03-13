use crate::workflows::wasm_engine::WasmEngine;
use mockall::automock;
use sx_types::{
    dna::Dna,
    entry::Entry,
    error::SkunkResult,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};

#[automock]
pub trait RibosomeT: Sized {
    fn run_validation(self, _entry: Entry) -> ValidationResult {
        // TODO: turn entry into "data"
        self.run_callback(())
    }

    fn run_callback(self, data: ()) -> ValidationResult;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    ///
    /// Note: it would be nice to pass the bundle by value and then return it at the end,
    /// but automock doesn't support lifetimes that appear in return values
    fn call_zome_function<'env>(
        self,
        bundle: &mut SourceChainCommitBundle<'env>,
        invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<ZomeInvocationResponse>;
}

/// Total hack just to have something to look at
/// The only WasmRibosome is a Wasm ribosome.
pub struct WasmRibosome {
    _engine: WasmEngine,
}

impl WasmRibosome {
    pub fn new(_dna: Dna) -> Self {
        Self {
            _engine: WasmEngine,
        }
    }
}

impl RibosomeT for WasmRibosome {

    fn run_callback(self, _data: ()) -> ValidationResult {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        _bundle: &mut SourceChainCommitBundle<'env>,
        _invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<ZomeInvocationResponse> {
        unimplemented!()
    }
}
