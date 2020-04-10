# test utils: wasm

here we have:

- several rust crates that compile to wasms that are useful for testing
- a `TestWasm` enum that maps variants to populated `DnaWasm` structs (bytes)
- a `build.rs` file that builds all the wasms for testing

these wasms _directly_ test the host/guest implementation of holochain

i.e. there is no expectation that the HDK or similar "high level" tooling will
be included.

it should be reasonably easy to interact with holochain even without the HDK
using the `holochain_wasmer_*` crates.

the tests themselves generally sit in the ribosome module in core.
this is necessary so that core can inject the host import functions.
