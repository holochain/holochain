# Wasm test utilities

This crate contains:

- several small crates that compile to Wasm and are used as test values.
- `enum TestWasm` which enumerates all of those crates.
-  `impl From<TestWasm> for DnaWasm` to obtain the compiled Wasm artifacts for those crates.
- a `build.rs` file that builds all those crates for compile-time inclusion in the library.

these wasms _directly_ test the host/guest implementation of holochain

i.e. there is no expectation that the HDK or similar "high level" tooling will
be included.

it should be reasonably easy to interact with holochain even without the HDK
using the `holochain_wasmer_*` crates.

The tests that run this Wasm generally sit in the [`ribosome.rs` module in core][ribosome]. This is necessary because the Wasm crates depend on certain global functions that core defines and needs to inject.

[ribosome]: https://github.com/Holo-Host/holochain-2020/blob/2b83a9340fba999e8c32adb9c342bd268f0ef480/crates/holochain/src/core/ribosome.rs
