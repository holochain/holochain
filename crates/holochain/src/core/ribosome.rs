//! A Ribosome is a structure which knows how to execute hApp code.
//!
//! We have only one instance of this: [WasmRibosome]. The abstract trait exists
//! so that we can write mocks against the RibosomeT interface, as well as
//! opening the possiblity that we might support applications written in other
//! languages and environments.

// This allow is here because #[automock] automaticaly creates a struct without
// documentation, and there seems to be no way to add docs to it after the fact
#![allow(missing_docs)]

use super::state::source_chain::SourceChain;
use crate::core::wasm_engine::WasmEngine;
use mockall::automock;
use sx_state::prelude::Reader;
use sx_types::{
    dna::Dna,
    entry::Entry,
    error::SkunkResult,
    nucleus::{ZomeInvocation, ZomeInvocationResponse},
    shims::*,
};

/// Represents a type which has not been decided upon yet
pub struct Todo;

/// Interface for a Ribosome. Currently used only for mocking, as our only
/// real concrete type is [WasmRibosome]
#[automock]
pub trait RibosomeT: Sized {
    /// Helper function for running a validation callback. Just calls
    /// run_callback under the hood.
    fn run_validation(self, _entry: Entry) -> ValidationResult {
        unimplemented!()
    }

    /// Run a callback function defined in a zome.
    ///
    /// This is differentiated from calling a zome function, even though in both
    /// cases it amounts to a FFI call of a guest function.
    fn run_callback(self, data: ()) -> Todo;

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    ///
    /// Note: it would be nice to pass the bundle by value and then return it at the end,
    /// but automock doesn't support lifetimes that appear in return values
    fn call_zome_function<'env>(
        self,
        source_chain: &mut SourceChain<'env, Reader<'env>>,
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
    /// Create a new instance
    pub fn new(_dna: Dna) -> Self {
        Self {
            _engine: WasmEngine,
        }
    }
}

impl RibosomeT for WasmRibosome {
    fn run_callback(self, _data: ()) -> Todo {
        unimplemented!()
    }

    /// Runs the specified zome fn. Returns the cursor used by HDK,
    /// so that it can be passed on to source chain manager for transactional writes
    fn call_zome_function<'env>(
        self,
        _bundle: &mut SourceChain<'env, Reader<'env>>,
        _invocation: ZomeInvocation,
        // source_chain: SourceChain,
    ) -> SkunkResult<ZomeInvocationResponse> {
        unimplemented!()
    }
}
