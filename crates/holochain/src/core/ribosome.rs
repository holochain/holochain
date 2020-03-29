use holochain_wasmer_host::WasmError;
use mockall::automock;
use sx_types::{
    dna::{wasm::DnaWasm, Dna},
    entry::Entry,
    error::SkunkResult,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};
use wasmer_runtime::{imports, Instance};

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
    _dna: Dna,
}

impl WasmRibosome {
    pub fn new(_dna: Dna) -> Self {
        Self { _dna }
    }

    fn instance(wasm: DnaWasm) -> Result<Instance, WasmError> {
        let imports = imports! {};
        holochain_wasmer_host::instantiate::instantiate(&wasm.code, &wasm.code, &imports)
    }

    fn imports() -> ImportObject {
        // let commit_entry_arc = Arc::new(self);
        // let commit_entry_closure = move |ctx: &Ctx, allocation_ptr: RemotePtr| {
        //     commit_entry(Arc::clone(conductor_api));
        // };
        //
        // let imports = imports! {
        //     "env" => {
        //         "commit_entry" => func!(commit_entry_closure),
        //         "some_other_fn" => func!(closure2)
        //     }
        // };
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
        cell_conductor_api: CellConductorApi,
        // _bundle: &mut SourceChainCommitBundle<'env>,
        _invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<ZomeInvocationResponse> {
        let dna = self.dna;
        let wasm = dna.get_wasm(zome_name);

        let payload: SerializedBytes = invocation.payload;

        let instance = self.instance(&wasm, &wasm, &imports);

        holochain_wasmer_host::guest::call(instance, "fn_to_call", payload);
    }
}
